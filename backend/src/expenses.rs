use axum::{Json, Router, extract::State, http::StatusCode, routing::get};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    AppState,
    auth::CurrentUser,
    db::now_ts,
    error::{ApiError, ApiResult},
};

pub fn router() -> Router<AppState> {
    Router::new().route("/expenses", get(list_expenses).post(create_expense))
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ExpenseEntry {
    pub id: String,
    pub user_id: String,
    pub display_name: String,
    pub created_by_user_id: String,
    pub created_by_display_name: String,
    pub description: String,
    pub amount_cents: i64,
    pub logged_at: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ExpenseSummary {
    pub user_id: String,
    pub display_name: String,
    pub outstanding_amount_cents: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ExpensesResponse {
    pub entries: Vec<ExpenseEntry>,
    pub summaries: Vec<ExpenseSummary>,
}

#[derive(Deserialize, ToSchema)]
pub struct CreateExpenseRequest {
    pub description: String,
    pub amount_cents: i64,
}

#[utoipa::path(get, path = "/api/v1/expenses", responses((status = 200, body = ExpensesResponse)))]
pub(crate) async fn list_expenses(
    State(state): State<AppState>,
    user: CurrentUser,
) -> ApiResult<Json<ExpensesResponse>> {
    ensure_stock_manager(&user)?;

    Ok(Json(ExpensesResponse {
        entries: load_expense_entries(&state).await?,
        summaries: load_expense_summaries(&state).await?,
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/expenses",
    request_body = CreateExpenseRequest,
    responses((status = 201, body = ExpenseEntry))
)]
pub(crate) async fn create_expense(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(input): Json<CreateExpenseRequest>,
) -> ApiResult<(StatusCode, Json<ExpenseEntry>)> {
    ensure_stock_manager(&user)?;
    let description = normalize_description(&input.description)?;
    validate_amount(input.amount_cents)?;

    let now = now_ts();
    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO expense_entries
         (id, user_id, created_by_user_id, description, amount_cents, logged_at, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&user.id)
    .bind(&user.id)
    .bind(&description)
    .bind(input.amount_cents)
    .bind(now)
    .bind(now)
    .execute(&state.db)
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(ExpenseEntry {
            id,
            user_id: user.id.clone(),
            display_name: user.display_name.clone(),
            created_by_user_id: user.id,
            created_by_display_name: user.display_name,
            description,
            amount_cents: input.amount_cents,
            logged_at: now,
        }),
    ))
}

pub async fn chair_expense_balance_cents(state: &AppState) -> ApiResult<i64> {
    Ok(-all_outstanding_expense_amount_cents(state).await?)
}

pub async fn user_expense_balance_cents(state: &AppState, user_id: &str) -> ApiResult<i64> {
    outstanding_amount_cents(state, user_id).await
}

pub async fn user_expense_total_cents(state: &AppState, user_id: &str) -> ApiResult<i64> {
    let (amount,): (Option<i64>,) = sqlx::query_as(
        "SELECT SUM(amount_cents)
         FROM expense_entries
         WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;
    Ok(amount.unwrap_or(0))
}

pub async fn user_received_payment_total_cents(state: &AppState, user_id: &str) -> ApiResult<i64> {
    let (amount,): (Option<i64>,) = sqlx::query_as(
        "SELECT SUM(amount_cents)
         FROM payments
         WHERE received_by_user_id = ?",
    )
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;
    Ok(amount.unwrap_or(0))
}

async fn all_outstanding_expense_amount_cents(state: &AppState) -> ApiResult<i64> {
    let (sum,): (Option<i64>,) = sqlx::query_as(
        "SELECT
           COALESCE((SELECT SUM(e.amount_cents) FROM expense_entries e), 0)
           - COALESCE((SELECT SUM(p.amount_cents)
                       FROM payments p
                       WHERE p.received_by_user_id IS NOT NULL), 0)",
    )
    .fetch_one(&state.db)
    .await?;
    Ok(sum.unwrap_or(0))
}

async fn load_expense_entries(state: &AppState) -> ApiResult<Vec<ExpenseEntry>> {
    sqlx::query_as::<_, ExpenseEntryRow>(
        "SELECT e.id, e.user_id, u.display_name, e.created_by_user_id,
                creator.display_name AS created_by_display_name, e.description,
                e.amount_cents, e.logged_at
         FROM expense_entries e
         JOIN users u ON u.id = e.user_id
         JOIN users creator ON creator.id = e.created_by_user_id
         ORDER BY e.logged_at DESC, e.created_at DESC",
    )
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|row| {
        Ok(ExpenseEntry {
            id: row.id,
            user_id: row.user_id,
            display_name: row.display_name,
            created_by_user_id: row.created_by_user_id,
            created_by_display_name: row.created_by_display_name,
            description: row.description,
            amount_cents: row.amount_cents,
            logged_at: row.logged_at,
        })
    })
    .collect()
}

async fn load_expense_summaries(state: &AppState) -> ApiResult<Vec<ExpenseSummary>> {
    sqlx::query_as::<_, ExpenseSummaryRow>(
        "SELECT u.id AS user_id,
                u.display_name,
                COALESCE((
                  SELECT SUM(e.amount_cents)
                  FROM expense_entries e
                  WHERE e.user_id = u.id
                ), 0)
                - COALESCE((
                  SELECT SUM(p.amount_cents)
                  FROM payments p
                  WHERE p.received_by_user_id = u.id
                ), 0) AS outstanding_amount_cents
         FROM users u
         WHERE u.stock_manager = 1
            OR EXISTS (SELECT 1 FROM expense_entries e WHERE e.user_id = u.id)
            OR EXISTS (SELECT 1 FROM payments p WHERE p.received_by_user_id = u.id)
         ORDER BY outstanding_amount_cents DESC, u.display_name ASC",
    )
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .filter(|row| row.outstanding_amount_cents != 0)
    .map(|row| {
        Ok(ExpenseSummary {
            user_id: row.user_id,
            display_name: row.display_name,
            outstanding_amount_cents: row.outstanding_amount_cents,
        })
    })
    .collect()
}

async fn outstanding_amount_cents(state: &AppState, user_id: &str) -> ApiResult<i64> {
    let (amount,): (Option<i64>,) = sqlx::query_as(
        "SELECT
           COALESCE((SELECT SUM(amount_cents)
                     FROM expense_entries
                     WHERE user_id = ?), 0)
           - COALESCE((SELECT SUM(amount_cents)
                       FROM payments
                       WHERE received_by_user_id = ?), 0)",
    )
    .bind(user_id)
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;
    Ok(amount.unwrap_or(0))
}

fn ensure_stock_manager(user: &CurrentUser) -> ApiResult<()> {
    if user.stock_manager {
        Ok(())
    } else {
        Err(ApiError::Forbidden)
    }
}

fn normalize_description(description: &str) -> ApiResult<String> {
    let description = description.trim();
    if description.is_empty() {
        return Err(ApiError::BadRequest("description must not be empty".into()));
    }
    Ok(description.to_string())
}

fn validate_amount(amount_cents: i64) -> ApiResult<()> {
    if amount_cents == 0 {
        return Err(ApiError::BadRequest("amount_cents must not be zero".into()));
    }
    Ok(())
}

#[derive(sqlx::FromRow)]
struct ExpenseEntryRow {
    id: String,
    user_id: String,
    display_name: String,
    created_by_user_id: String,
    created_by_display_name: String,
    description: String,
    amount_cents: i64,
    logged_at: i64,
}

#[derive(sqlx::FromRow)]
struct ExpenseSummaryRow {
    user_id: String,
    display_name: String,
    outstanding_amount_cents: i64,
}

#[cfg(test)]
mod tests {
    use axum::{Json, extract::State, http::StatusCode};
    use uuid::Uuid;

    use super::*;
    use crate::{config::Config, db::connect};

    fn user(id: &str) -> CurrentUser {
        CurrentUser {
            id: id.to_string(),
            user_type: "member".into(),
            gitlab_id: Some(1),
            display_name: id.to_string(),
            leaderboard_opt_in: true,
            achievement_sharing_opt_out: false,
            mattermost_opt_in: false,
            stock_manager: id != "regular",
            paypal_link: None,
        }
    }

    async fn state() -> AppState {
        let db_path = format!("/tmp/coffee-expenses-{}.db", Uuid::new_v4());
        let db = connect(&format!("sqlite://{db_path}")).await.unwrap();
        sqlx::query(
            "INSERT INTO users
             (id, user_type, gitlab_id, display_name, leaderboard_opt_in,
              achievement_sharing_opt_out, mattermost_opt_in, stock_manager, created_at, updated_at)
             VALUES
             ('actor', 'member', 1, 'Actor', 1, 0, 0, 1, 1, 1),
             ('other', 'member', 2, 'Other', 1, 0, 0, 1, 1, 1),
             ('regular', 'member', 3, 'Regular', 1, 0, 0, 0, 1, 1)",
        )
        .execute(&db)
        .await
        .unwrap();
        AppState {
            config: Config {
                app_name: "Coffee Tracker".into(),
                database_url: format!("sqlite://{db_path}"),
                bind_addr: "127.0.0.1:0".parse().unwrap(),
                session_secret: "test".into(),
                gitlab_base_url: "https://gitlab.example.org".into(),
                gitlab_oauth_client_id: String::new(),
                gitlab_oauth_client_secret: String::new(),
                gitlab_oauth_scopes: "read_user".into(),
                authentik_issuer_url: String::new(),
                authentik_oauth_client_id: String::new(),
                authentik_oauth_client_secret: String::new(),
                authentik_oauth_scopes: "openid email profile groups".into(),
                auth_allowed_groups: Vec::new(),
                auth_staff_groups: Vec::new(),
                public_base_url: "http://localhost".into(),
                frontend_base_url: "http://localhost".into(),
                mattermost_webhook_url: None,
                matrix_webhook_url: None,
                smtp: None,
                paypal_link: None,
                timezone: "UTC".into(),
                session_ttl: std::time::Duration::from_secs(60),
            },
            db,
            http: reqwest::Client::new(),
        }
    }

    async fn insert_expense(state: &AppState, id: &str, user_id: &str, amount_cents: i64, at: i64) {
        sqlx::query(
            "INSERT INTO expense_entries
             (id, user_id, created_by_user_id, description, amount_cents, logged_at, created_at)
             VALUES (?, ?, 'actor', 'Coffee beans', ?, ?, ?)",
        )
        .bind(id)
        .bind(user_id)
        .bind(amount_cents)
        .bind(at)
        .bind(at)
        .execute(&state.db)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn only_staff_can_list_or_create_expenses() {
        let state = state().await;
        let err = list_expenses(State(state.clone()), user("regular"))
            .await
            .unwrap_err();
        assert!(matches!(err, ApiError::Forbidden));

        let err = create_expense(
            State(state),
            user("regular"),
            Json(CreateExpenseRequest {
                description: "Coffee".into(),
                amount_cents: 100,
            }),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, ApiError::Forbidden));
    }

    #[tokio::test]
    async fn create_expense_trims_description_and_rejects_zero_amount() {
        let state = state().await;
        let (status, Json(entry)) = create_expense(
            State(state.clone()),
            user("actor"),
            Json(CreateExpenseRequest {
                description: "  bought coffee  ".into(),
                amount_cents: 1234,
            }),
        )
        .await
        .unwrap();
        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(entry.description, "bought coffee");
        assert_eq!(entry.amount_cents, 1234);

        let err = create_expense(
            State(state),
            user("actor"),
            Json(CreateExpenseRequest {
                description: "Coffee".into(),
                amount_cents: 0,
            }),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, ApiError::BadRequest(_)));
    }

    #[tokio::test]
    async fn positive_and_negative_entries_roll_into_staff_and_chair_balances() {
        let state = state().await;
        insert_expense(&state, "positive", "actor", 2500, 10).await;
        insert_expense(&state, "negative", "actor", -500, 20).await;
        insert_expense(&state, "other", "other", 1000, 30).await;
        sqlx::query("UPDATE users SET stock_manager = 0 WHERE id = 'other'")
            .execute(&state.db)
            .await
            .unwrap();

        let Json(response) = list_expenses(State(state.clone()), user("actor"))
            .await
            .unwrap();
        let actor = response
            .summaries
            .iter()
            .find(|summary| summary.user_id == "actor")
            .unwrap();
        let former_staff = response
            .summaries
            .iter()
            .find(|summary| summary.user_id == "other")
            .unwrap();
        assert_eq!(actor.outstanding_amount_cents, 2000);
        assert_eq!(former_staff.outstanding_amount_cents, 1000);
        assert_eq!(chair_expense_balance_cents(&state).await.unwrap(), -3000);
    }

    #[tokio::test]
    async fn payments_received_reduce_staff_expense_balances() {
        let state = state().await;
        insert_expense(&state, "expense", "actor", 5000, 10).await;
        sqlx::query(
            "INSERT INTO payments
             (id, user_id, received_by_user_id, amount_cents, drink_count, period_end_ts,
              paid_at, created_at)
             VALUES ('assigned', 'regular', 'actor', 1250, 25, 20, 20, 20)",
        )
        .execute(&state.db)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO payments
             (id, user_id, amount_cents, drink_count, period_end_ts, paid_at, created_at)
             VALUES ('legacy', 'regular', 2500, 50, 30, 30, 30)",
        )
        .execute(&state.db)
        .await
        .unwrap();

        let Json(response) = list_expenses(State(state.clone()), user("actor"))
            .await
            .unwrap();
        let actor = response
            .summaries
            .iter()
            .find(|summary| summary.user_id == "actor")
            .unwrap();

        assert_eq!(actor.outstanding_amount_cents, 3750);
        assert_eq!(
            user_expense_balance_cents(&state, "actor").await.unwrap(),
            3750
        );
        assert_eq!(chair_expense_balance_cents(&state).await.unwrap(), -3750);
    }

    #[tokio::test]
    async fn summaries_exclude_staff_without_expense_values() {
        let state = state().await;
        insert_expense(&state, "expense", "actor", 2500, 10).await;

        let Json(response) = list_expenses(State(state), user("actor")).await.unwrap();

        assert_eq!(response.summaries.len(), 1);
        assert_eq!(response.summaries[0].user_id, "actor");
    }
}
