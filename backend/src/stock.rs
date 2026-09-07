use axum::{
    Json, Router,
    extract::{Path, State},
    routing::{get, patch},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    AppState,
    auth::CurrentUser,
    db::now_ts,
    error::{ApiError, ApiResult},
    notifications::notify_stock_low,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/stock", get(list_stock))
        .route("/stock/{item_id}", patch(update_stock))
}

#[derive(Debug, Clone, Serialize, ToSchema, sqlx::FromRow)]
pub struct StockItem {
    pub id: String,
    pub name: String,
    pub unit: String,
    pub current_amount: f64,
    pub critical_amount: f64,
    pub decrement_per_log: f64,
    pub alerted: bool,
    pub updated_at: i64,
}

#[derive(Serialize, ToSchema)]
pub struct StockResponse {
    pub items: Vec<StockItem>,
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateStockRequest {
    pub current_amount: Option<f64>,
    pub critical_amount: Option<f64>,
    pub decrement_per_log: Option<f64>,
    pub alerted: Option<bool>,
}

#[utoipa::path(get, path = "/api/v1/stock", responses((status = 200, body = StockResponse)))]
pub(crate) async fn list_stock(
    State(state): State<AppState>,
    _user: CurrentUser,
) -> ApiResult<Json<StockResponse>> {
    Ok(Json(StockResponse {
        items: load_stock_items(&state).await?,
    }))
}

#[utoipa::path(
    patch,
    path = "/api/v1/stock/{item_id}",
    params(("item_id" = String, Path)),
    request_body = UpdateStockRequest,
    responses((status = 200, body = StockItem))
)]
pub(crate) async fn update_stock(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(item_id): Path<String>,
    Json(input): Json<UpdateStockRequest>,
) -> ApiResult<Json<StockItem>> {
    ensure_stock_manager(&user)?;
    validate_non_negative(input.current_amount, "current_amount")?;
    validate_non_negative(input.critical_amount, "critical_amount")?;
    validate_non_negative(input.decrement_per_log, "decrement_per_log")?;

    let mut item = load_stock_item(&state, &item_id).await?;
    if let Some(value) = input.current_amount {
        item.current_amount = normalize_amount(value);
    }
    if let Some(value) = input.critical_amount {
        item.critical_amount = normalize_amount(value);
    }
    if let Some(value) = input.decrement_per_log {
        item.decrement_per_log = normalize_amount(value);
    }
    if let Some(value) = input.alerted {
        item.alerted = value;
    }
    if item.current_amount > item.critical_amount {
        item.alerted = false;
    }

    let now = now_ts();
    sqlx::query(
        "UPDATE stock_items
         SET current_amount = ?, critical_amount = ?, decrement_per_log = ?,
             alerted = ?, updated_at = ?
         WHERE id = ?",
    )
    .bind(item.current_amount)
    .bind(item.critical_amount)
    .bind(item.decrement_per_log)
    .bind(item.alerted as i64)
    .bind(now)
    .bind(&item.id)
    .execute(&state.db)
    .await?;

    Ok(Json(load_stock_item(&state, &item_id).await?))
}

pub async fn decrement_for_beverage(state: &AppState, beverage_id: &str) -> ApiResult<()> {
    let Some(stock_id) = stock_item_for_beverage(beverage_id) else {
        return Ok(());
    };
    let item = load_stock_item(state, stock_id).await?;
    if item.decrement_per_log <= 0.0 {
        return Ok(());
    }

    let previous = item.current_amount;
    let next = normalize_amount((previous - item.decrement_per_log).max(0.0));
    let crossed_threshold = previous > item.critical_amount && next <= item.critical_amount;
    let should_alert = crossed_threshold && !item.alerted;
    let now = now_ts();

    sqlx::query(
        "UPDATE stock_items
         SET current_amount = ?, alerted = ?, updated_at = ?
         WHERE id = ?",
    )
    .bind(next)
    .bind((item.alerted || should_alert) as i64)
    .bind(now)
    .bind(&item.id)
    .execute(&state.db)
    .await?;

    if should_alert {
        notify_stock_low(state, &item.name, next, item.critical_amount, &item.unit).await;
    }

    Ok(())
}

fn stock_item_for_beverage(beverage_id: &str) -> Option<&'static str> {
    match beverage_id {
        "coffee" => Some("beans"),
        "espresso_pads" => Some("espresso_pads"),
        "tea" => Some("tea"),
        _ => None,
    }
}

fn ensure_stock_manager(user: &CurrentUser) -> ApiResult<()> {
    if user.stock_manager {
        Ok(())
    } else {
        Err(ApiError::Forbidden)
    }
}

fn validate_non_negative(value: Option<f64>, field: &str) -> ApiResult<()> {
    if value.is_some_and(|value| !value.is_finite() || value < 0.0) {
        return Err(ApiError::BadRequest(format!(
            "{field} must be a finite non-negative number"
        )));
    }
    Ok(())
}

fn normalize_amount(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

async fn load_stock_items(state: &AppState) -> ApiResult<Vec<StockItem>> {
    sqlx::query_as::<_, StockItem>(
        "SELECT id, name, unit, current_amount, critical_amount,
                decrement_per_log, alerted, updated_at
         FROM stock_items
         ORDER BY CASE id WHEN 'beans' THEN 0 WHEN 'espresso_pads' THEN 1 WHEN 'tea' THEN 2 ELSE 3 END, name ASC",
    )
    .fetch_all(&state.db)
    .await
    .map_err(ApiError::from)
}

async fn load_stock_item(state: &AppState, item_id: &str) -> ApiResult<StockItem> {
    sqlx::query_as::<_, StockItem>(
        "SELECT id, name, unit, current_amount, critical_amount,
                decrement_per_log, alerted, updated_at
         FROM stock_items
         WHERE id = ?",
    )
    .bind(item_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(ApiError::NotFound)
}

#[cfg(test)]
mod tests {
    use axum::extract::State;
    use uuid::Uuid;

    use super::*;
    use crate::{config::Config, db::connect};

    fn user(stock_manager: bool) -> CurrentUser {
        CurrentUser {
            id: "user".into(),
            user_type: "member".into(),
            gitlab_id: Some(1),
            display_name: "User".into(),
            leaderboard_opt_in: true,
            achievement_sharing_opt_out: false,
            mattermost_opt_in: false,
            stock_manager,
            paypal_link: None,
        }
    }

    async fn state() -> AppState {
        let db_path = format!("/tmp/coffee-stock-{}.db", Uuid::new_v4());
        let db = connect(&format!("sqlite://{db_path}")).await.unwrap();
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

    #[tokio::test]
    async fn stock_decrement_clamps_at_zero() {
        let state = state().await;
        let _ = update_stock(
            State(state.clone()),
            user(true),
            Path("espresso_pads".into()),
            Json(UpdateStockRequest {
                current_amount: Some(2.0),
                critical_amount: Some(1.0),
                decrement_per_log: Some(5.0),
                alerted: Some(false),
            }),
        )
        .await
        .unwrap();

        decrement_for_beverage(&state, "espresso_pads")
            .await
            .unwrap();
        let item = load_stock_item(&state, "espresso_pads").await.unwrap();
        assert_eq!(item.current_amount, 0.0);
        assert!(item.alerted);
    }

    #[tokio::test]
    async fn restocking_above_threshold_resets_alert_state() {
        let state = state().await;
        let Json(item) = update_stock(
            State(state),
            user(true),
            Path("beans".into()),
            Json(UpdateStockRequest {
                current_amount: Some(2.0),
                critical_amount: Some(0.5),
                decrement_per_log: None,
                alerted: Some(true),
            }),
        )
        .await
        .unwrap();
        assert!(!item.alerted);
    }

    #[tokio::test]
    async fn low_stock_alert_state_stays_set_while_stock_remains_low() {
        let state = state().await;
        let _ = update_stock(
            State(state.clone()),
            user(true),
            Path("espresso_pads".into()),
            Json(UpdateStockRequest {
                current_amount: Some(2.0),
                critical_amount: Some(1.0),
                decrement_per_log: Some(1.0),
                alerted: Some(false),
            }),
        )
        .await
        .unwrap();

        decrement_for_beverage(&state, "espresso_pads")
            .await
            .unwrap();
        let item = load_stock_item(&state, "espresso_pads").await.unwrap();
        assert!(item.alerted);

        decrement_for_beverage(&state, "espresso_pads")
            .await
            .unwrap();
        let item = load_stock_item(&state, "espresso_pads").await.unwrap();
        assert!(item.alerted);
        assert_eq!(item.current_amount, 0.0);
    }

    #[tokio::test]
    async fn stock_decrement_rounds_to_thousandths() {
        let state = state().await;
        let _ = update_stock(
            State(state.clone()),
            user(true),
            Path("espresso_pads".into()),
            Json(UpdateStockRequest {
                current_amount: Some(1.0),
                critical_amount: Some(0.0),
                decrement_per_log: Some(0.2),
                alerted: Some(false),
            }),
        )
        .await
        .unwrap();

        decrement_for_beverage(&state, "espresso_pads")
            .await
            .unwrap();
        decrement_for_beverage(&state, "espresso_pads")
            .await
            .unwrap();
        decrement_for_beverage(&state, "espresso_pads")
            .await
            .unwrap();
        decrement_for_beverage(&state, "espresso_pads")
            .await
            .unwrap();

        let item = load_stock_item(&state, "espresso_pads").await.unwrap();
        assert_eq!(item.current_amount, 0.2);
    }

    #[tokio::test]
    async fn tea_decrements_loose_tea_stock() {
        let state = state().await;
        let _ = update_stock(
            State(state.clone()),
            user(true),
            Path("tea".into()),
            Json(UpdateStockRequest {
                current_amount: Some(0.05),
                critical_amount: Some(0.0),
                decrement_per_log: Some(0.01),
                alerted: Some(false),
            }),
        )
        .await
        .unwrap();

        decrement_for_beverage(&state, "tea").await.unwrap();

        let item = load_stock_item(&state, "tea").await.unwrap();
        assert_eq!(item.current_amount, 0.04);
    }

    #[tokio::test]
    async fn only_stock_managers_can_patch_stock() {
        let state = state().await;
        let result = update_stock(
            State(state),
            user(false),
            Path("beans".into()),
            Json(UpdateStockRequest {
                current_amount: Some(1.0),
                critical_amount: None,
                decrement_per_log: None,
                alerted: None,
            }),
        )
        .await;
        assert!(matches!(result, Err(ApiError::Forbidden)));
    }
}
