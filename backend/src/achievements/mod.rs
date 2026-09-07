use std::collections::HashSet;

use axum::{Json, Router, extract::State, routing::get};
use chrono::{Datelike, Duration, NaiveDate, TimeZone, Timelike, Utc, Weekday};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    AppState,
    auth::CurrentUser,
    error::{ApiError, ApiResult},
    stats::{caffeine_decay, resolve_tz},
};

pub fn router() -> Router<AppState> {
    Router::new().route("/achievements/me", get(my_achievements))
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct AchievementEvent {
    pub id: String,
    pub name: String,
    pub description: String,
}

#[derive(Serialize, ToSchema)]
pub struct AchievementItem {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub hint: Option<String>,
    pub unlocked: bool,
    pub unlocked_at: Option<i64>,
}

#[derive(Serialize, ToSchema)]
pub struct AchievementsResponse {
    pub achievements: Vec<AchievementItem>,
}

#[derive(sqlx::FromRow)]
struct AchievementDefinition {
    id: String,
    name: String,
    description: String,
    criteria_kind: String,
    threshold: Option<i64>,
}

#[utoipa::path(
    get,
    path = "/api/v1/achievements/me",
    responses((status = 200, body = AchievementsResponse))
)]
pub(crate) async fn my_achievements(
    State(state): State<AppState>,
    user: CurrentUser,
) -> ApiResult<Json<AchievementsResponse>> {
    let rows = sqlx::query_as::<_, AchievementWithUnlock>(
        "SELECT d.id, d.name, d.description, d.hint, u.unlocked_at
         FROM achievement_definitions d
         LEFT JOIN achievement_unlocks u
           ON u.achievement_id = d.id AND u.user_id = ?
         WHERE d.active = 1
         ORDER BY d.created_at ASC",
    )
    .bind(&user.id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(AchievementsResponse {
        achievements: rows
            .into_iter()
            .map(|row| AchievementItem {
                id: row.id,
                name: row.name,
                description: row.unlocked_at.map(|_| row.description),
                hint: row.unlocked_at.is_none().then_some(row.hint),
                unlocked: row.unlocked_at.is_some(),
                unlocked_at: row.unlocked_at,
            })
            .collect(),
    }))
}

pub async fn evaluate_after_consumption(
    state: &AppState,
    user: &CurrentUser,
    consumption_id: &str,
    now: i64,
) -> ApiResult<Vec<AchievementEvent>> {
    let definitions = sqlx::query_as::<_, AchievementDefinition>(
        "SELECT id, name, description, criteria_kind, threshold
         FROM achievement_definitions
         WHERE active = 1",
    )
    .fetch_all(&state.db)
    .await?;

    let mut unlocked = Vec::new();
    for definition in definitions {
        if is_unlocked(state, &user.id, &definition.id).await? {
            continue;
        }
        if criteria_met(state, &user.id, consumption_id, &definition, now).await? {
            sqlx::query(
                "INSERT OR IGNORE INTO achievement_unlocks
                 (id, user_id, achievement_id, unlocked_at)
                 VALUES (?, ?, ?, ?)",
            )
            .bind(Uuid::new_v4().to_string())
            .bind(&user.id)
            .bind(&definition.id)
            .bind(now)
            .execute(&state.db)
            .await?;
            unlocked.push(AchievementEvent {
                id: definition.id,
                name: definition.name,
                description: definition.description,
            });
        }
    }

    Ok(unlocked)
}

pub async fn evaluate_after_payment(
    state: &AppState,
    user_id: &str,
    paid_amount_cents: i64,
    now: i64,
) -> ApiResult<Vec<AchievementEvent>> {
    if is_guest_user(state, user_id).await? {
        return Ok(Vec::new());
    }

    let definitions = sqlx::query_as::<_, AchievementDefinition>(
        "SELECT id, name, description, criteria_kind, threshold
         FROM achievement_definitions
         WHERE active = 1",
    )
    .fetch_all(&state.db)
    .await?;

    let mut unlocked = Vec::new();
    for definition in definitions {
        if is_unlocked(state, user_id, &definition.id).await? {
            continue;
        }
        if payment_criteria_met(state, user_id, &definition, paid_amount_cents).await? {
            sqlx::query(
                "INSERT OR IGNORE INTO achievement_unlocks
                 (id, user_id, achievement_id, unlocked_at)
                 VALUES (?, ?, ?, ?)",
            )
            .bind(Uuid::new_v4().to_string())
            .bind(user_id)
            .bind(&definition.id)
            .bind(now)
            .execute(&state.db)
            .await?;
            unlocked.push(AchievementEvent {
                id: definition.id,
                name: definition.name,
                description: definition.description,
            });
        }
    }

    Ok(unlocked)
}

async fn is_unlocked(state: &AppState, user_id: &str, achievement_id: &str) -> ApiResult<bool> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM achievement_unlocks WHERE user_id = ? AND achievement_id = ?",
    )
    .bind(user_id)
    .bind(achievement_id)
    .fetch_optional(&state.db)
    .await?;
    Ok(row.is_some())
}

async fn criteria_met(
    state: &AppState,
    user_id: &str,
    consumption_id: &str,
    definition: &AchievementDefinition,
    now: i64,
) -> ApiResult<bool> {
    let threshold = definition.threshold.unwrap_or(1);
    if let Some(beverage_id) = definition.criteria_kind.strip_prefix("beverage_count:") {
        let (count,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM consumptions WHERE user_id = ? AND beverage_id = ?",
        )
        .bind(user_id)
        .bind(beverage_id)
        .fetch_one(&state.db)
        .await?;
        return Ok(count >= threshold);
    }
    if let Some(hours) = hour_window(&definition.criteria_kind) {
        return Ok(
            consumption_in_hour_window(state, user_id, consumption_id, &hours).await?
                && count_consumptions_in_hour_window(state, user_id, &hours).await? >= threshold,
        );
    }

    match definition.criteria_kind.as_str() {
        "lifetime_cups" => {
            let (count,): (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM consumptions WHERE user_id = ? AND beverage_id = 'coffee'",
            )
            .bind(user_id)
            .fetch_one(&state.db)
            .await?;
            Ok(count >= threshold)
        }
        "early_bird" => consumption_in_hour_window(state, user_id, consumption_id, &(0..8)).await,
        "night_owl" => consumption_in_hour_window(state, user_id, consumption_id, &(22..24)).await,
        "streak_days" => Ok(longest_streak(state, user_id).await? >= threshold),
        "beverage_variety" => {
            let (count,): (i64,) = sqlx::query_as(
                "SELECT COUNT(DISTINCT beverage_id) FROM consumptions WHERE user_id = ?",
            )
            .bind(user_id)
            .fetch_one(&state.db)
            .await?;
            Ok(count >= threshold)
        }
        "regular_menu_variety" => {
            let (count,): (i64,) = sqlx::query_as(
                "SELECT COUNT(DISTINCT beverage_id)
                 FROM consumptions
                 WHERE user_id = ? AND beverage_id IN ('coffee', 'espresso_pads', 'eis', 'tea')",
            )
            .bind(user_id)
            .fetch_one(&state.db)
            .await?;
            Ok(count >= threshold)
        }
        "single_day_count" => Ok(max_single_day_count(state, user_id).await? >= threshold),
        "caffeine_threshold" => Ok(caffeine_level(state, user_id, now).await? >= threshold as f64),
        "unpaid_amount" => Ok(unpaid_amount_cents(state, user_id).await? >= threshold),
        "weekend_count" => Ok(
            consumption_is_weekend(state, user_id, consumption_id).await?
                && weekend_consumption_count(state, user_id).await? >= threshold,
        ),
        "return_after_gap_days" => {
            Ok(return_after_gap_days(state, user_id, consumption_id).await? >= threshold)
        }
        _ => Ok(false),
    }
}

async fn payment_criteria_met(
    state: &AppState,
    user_id: &str,
    definition: &AchievementDefinition,
    paid_amount_cents: i64,
) -> ApiResult<bool> {
    let threshold = definition.threshold.unwrap_or(1);
    match definition.criteria_kind.as_str() {
        "payment_count" => Ok(payment_count(state, user_id).await? >= threshold),
        "single_payment_amount" => Ok(paid_amount_cents >= threshold),
        _ => Ok(false),
    }
}

async fn is_guest_user(state: &AppState, user_id: &str) -> ApiResult<bool> {
    let row: Option<(String,)> = sqlx::query_as("SELECT user_type FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_optional(&state.db)
        .await?;
    Ok(matches!(
        row.as_ref().map(|(user_type,)| user_type.as_str()),
        Some("guest")
    ))
}

async fn consumption_in_hour_window(
    state: &AppState,
    user_id: &str,
    consumption_id: &str,
    hours: &std::ops::Range<u32>,
) -> ApiResult<bool> {
    let Some(dt) = consumption_local_datetime(state, user_id, consumption_id).await? else {
        return Ok(false);
    };
    Ok(hours.contains(&dt.hour()))
}

fn hour_window(criteria_kind: &str) -> Option<std::ops::Range<u32>> {
    let rest = criteria_kind.strip_prefix("hour_window_count:")?;
    let mut parts = rest.split(':');
    let start = parts.next()?.parse::<u32>().ok()?;
    let end = parts.next()?.parse::<u32>().ok()?;
    if parts.next().is_some() || start >= end || end > 24 {
        return None;
    }
    Some(start..end)
}

async fn count_consumptions_in_hour_window(
    state: &AppState,
    user_id: &str,
    hours: &std::ops::Range<u32>,
) -> ApiResult<i64> {
    let tz = resolve_tz(&state.config.timezone);
    let rows: Vec<(i64,)> =
        sqlx::query_as("SELECT consumed_at FROM consumptions WHERE user_id = ?")
            .bind(user_id)
            .fetch_all(&state.db)
            .await?;
    Ok(rows
        .into_iter()
        .filter(|(timestamp,)| {
            Utc.timestamp_opt(*timestamp, 0)
                .single()
                .map(|dt| hours.contains(&dt.with_timezone(&tz).hour()))
                .unwrap_or(false)
        })
        .count() as i64)
}

async fn weekend_consumption_count(state: &AppState, user_id: &str) -> ApiResult<i64> {
    let tz = resolve_tz(&state.config.timezone);
    let rows: Vec<(i64,)> =
        sqlx::query_as("SELECT consumed_at FROM consumptions WHERE user_id = ?")
            .bind(user_id)
            .fetch_all(&state.db)
            .await?;
    Ok(rows
        .into_iter()
        .filter(|(timestamp,)| {
            Utc.timestamp_opt(*timestamp, 0)
                .single()
                .map(|dt| matches!(dt.with_timezone(&tz).weekday(), Weekday::Sat | Weekday::Sun))
                .unwrap_or(false)
        })
        .count() as i64)
}

async fn consumption_is_weekend(
    state: &AppState,
    user_id: &str,
    consumption_id: &str,
) -> ApiResult<bool> {
    let Some(dt) = consumption_local_datetime(state, user_id, consumption_id).await? else {
        return Ok(false);
    };
    Ok(matches!(dt.weekday(), Weekday::Sat | Weekday::Sun))
}

async fn consumption_local_datetime(
    state: &AppState,
    user_id: &str,
    consumption_id: &str,
) -> ApiResult<Option<chrono::DateTime<chrono_tz::Tz>>> {
    let tz = resolve_tz(&state.config.timezone);
    let row: Option<(i64,)> =
        sqlx::query_as("SELECT consumed_at FROM consumptions WHERE id = ? AND user_id = ?")
            .bind(consumption_id)
            .bind(user_id)
            .fetch_optional(&state.db)
            .await?;
    Ok(row.and_then(|(timestamp,)| {
        Utc.timestamp_opt(timestamp, 0)
            .single()
            .map(|dt| dt.with_timezone(&tz))
    }))
}

async fn return_after_gap_days(
    state: &AppState,
    user_id: &str,
    consumption_id: &str,
) -> ApiResult<i64> {
    let tz = resolve_tz(&state.config.timezone);
    let current: Option<(i64,)> =
        sqlx::query_as("SELECT consumed_at FROM consumptions WHERE id = ? AND user_id = ?")
            .bind(consumption_id)
            .bind(user_id)
            .fetch_optional(&state.db)
            .await?;
    let Some((current_at,)) = current else {
        return Ok(0);
    };
    let previous: Option<(i64,)> = sqlx::query_as(
        "SELECT consumed_at
         FROM consumptions
         WHERE user_id = ? AND id != ? AND consumed_at < ?
         ORDER BY consumed_at DESC
         LIMIT 1",
    )
    .bind(user_id)
    .bind(consumption_id)
    .bind(current_at)
    .fetch_optional(&state.db)
    .await?;
    let Some((previous_at,)) = previous else {
        return Ok(0);
    };
    let Some(current_date) = Utc
        .timestamp_opt(current_at, 0)
        .single()
        .map(|dt| dt.with_timezone(&tz).date_naive())
    else {
        return Ok(0);
    };
    let Some(previous_date) = Utc
        .timestamp_opt(previous_at, 0)
        .single()
        .map(|dt| dt.with_timezone(&tz).date_naive())
    else {
        return Ok(0);
    };
    Ok(current_date.signed_duration_since(previous_date).num_days())
}

async fn longest_streak(state: &AppState, user_id: &str) -> ApiResult<i64> {
    let tz = resolve_tz(&state.config.timezone);
    let rows: Vec<(i64,)> =
        sqlx::query_as("SELECT consumed_at FROM consumptions WHERE user_id = ?")
            .bind(user_id)
            .fetch_all(&state.db)
            .await?;
    let mut dates = rows
        .into_iter()
        .filter_map(|(timestamp,)| Utc.timestamp_opt(timestamp, 0).single())
        .map(|dt| dt.with_timezone(&tz).date_naive())
        .filter(|date| is_lab_day(*date))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if dates.is_empty() {
        return Ok(0);
    }
    dates.sort_unstable();
    let mut longest = 1;
    let mut run = 1;
    for pair in dates.windows(2) {
        if pair[1] == next_lab_day(pair[0]) {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 1;
        }
    }
    Ok(longest)
}

fn is_lab_day(date: NaiveDate) -> bool {
    !matches!(date.weekday(), Weekday::Sat | Weekday::Sun)
}

fn next_lab_day(mut date: NaiveDate) -> NaiveDate {
    loop {
        date += Duration::days(1);
        if is_lab_day(date) {
            return date;
        }
    }
}

async fn max_single_day_count(state: &AppState, user_id: &str) -> ApiResult<i64> {
    let tz = resolve_tz(&state.config.timezone);
    let rows: Vec<(i64,)> =
        sqlx::query_as("SELECT consumed_at FROM consumptions WHERE user_id = ?")
            .bind(user_id)
            .fetch_all(&state.db)
            .await?;
    let mut counts = std::collections::HashMap::new();
    for (timestamp,) in rows {
        let date = Utc
            .timestamp_opt(timestamp, 0)
            .single()
            .ok_or_else(|| ApiError::BadRequest("invalid timestamp in database".into()))?
            .with_timezone(&tz)
            .date_naive();
        *counts.entry(date).or_insert(0_i64) += 1;
    }
    Ok(counts.values().copied().max().unwrap_or(0))
}

async fn caffeine_level(state: &AppState, user_id: &str, now: i64) -> ApiResult<f64> {
    let rows: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT c.consumed_at, b.caffeine_mg
         FROM consumptions c
         JOIN beverages b ON b.id = c.beverage_id
         WHERE c.user_id = ?",
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;
    Ok(rows
        .into_iter()
        .filter(|(consumed_at, caffeine_mg)| *caffeine_mg > 0 && *consumed_at <= now)
        .map(|(consumed_at, caffeine_mg)| caffeine_decay(caffeine_mg as f64, now - consumed_at))
        .sum())
}

async fn unpaid_amount_cents(state: &AppState, user_id: &str) -> ApiResult<i64> {
    let (max_period_end,): (Option<i64>,) =
        sqlx::query_as("SELECT MAX(period_end_ts) FROM payments WHERE user_id = ?")
            .bind(user_id)
            .fetch_one(&state.db)
            .await?;
    let (amount,): (Option<i64>,) = sqlx::query_as(
        "SELECT SUM(b.price_cents)
         FROM consumptions c
         JOIN beverages b ON b.id = c.beverage_id
         WHERE c.user_id = ? AND c.consumed_at >= ?",
    )
    .bind(user_id)
    .bind(max_period_end.unwrap_or(0))
    .fetch_one(&state.db)
    .await?;
    Ok(amount.unwrap_or(0))
}

async fn payment_count(state: &AppState, user_id: &str) -> ApiResult<i64> {
    let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM payments WHERE user_id = ?")
        .bind(user_id)
        .fetch_one(&state.db)
        .await?;
    Ok(count)
}

#[derive(sqlx::FromRow)]
struct AchievementWithUnlock {
    id: String,
    name: String,
    description: String,
    hint: String,
    unlocked_at: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{config::Config, db::connect};

    fn user() -> CurrentUser {
        CurrentUser {
            id: "user".into(),
            user_type: "member".into(),
            gitlab_id: Some(1),
            display_name: "User".into(),
            leaderboard_opt_in: true,
            achievement_sharing_opt_out: false,
            mattermost_opt_in: false,
            stock_manager: false,
            paypal_link: None,
        }
    }

    async fn state() -> AppState {
        let db_path = format!("/tmp/coffee-achievements-{}.db", Uuid::new_v4());
        let db = connect(&format!("sqlite://{db_path}")).await.unwrap();
        sqlx::query(
            "INSERT INTO users
             (id, user_type, gitlab_id, display_name, leaderboard_opt_in,
              achievement_sharing_opt_out, mattermost_opt_in, created_at, updated_at)
             VALUES ('user', 'member', 1, 'User', 1, 0, 0, 1, 1)",
        )
        .execute(&db)
        .await
        .unwrap();
        sqlx::query("INSERT INTO machines (id, name, created_at) VALUES ('main', 'Main', 1)")
            .execute(&db)
            .await
            .unwrap();
        sqlx::query(
            "INSERT OR IGNORE INTO beverages (id, name, price_cents, caffeine_mg, created_at)
             VALUES
                ('coffee', 'Coffee', 50, 90, 1),
                ('espresso_pads', 'Espresso (pads)', 50, 90, 1),
                ('eis', 'Eis', 50, 0, 1),
                ('tea', 'Tea', 25, 0, 1)",
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

    async fn insert_definition(state: &AppState, id: &str, criteria_kind: &str, threshold: i64) {
        sqlx::query(
            "INSERT INTO achievement_definitions
             (id, name, description, hint, criteria_kind, threshold, active, created_at)
             VALUES (?, ?, 'Description', 'Hint', ?, ?, 1, 1)",
        )
        .bind(id)
        .bind(id)
        .bind(criteria_kind)
        .bind(threshold)
        .execute(&state.db)
        .await
        .unwrap();
    }

    async fn insert_consumption(state: &AppState, id: &str, timestamp: i64) {
        sqlx::query(
            "INSERT INTO consumptions
             (id, user_id, machine_id, beverage_id, consumed_at, created_at)
             VALUES (?, 'user', 'main', 'coffee', ?, ?)",
        )
        .bind(id)
        .bind(timestamp)
        .bind(timestamp)
        .execute(&state.db)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn debt_achievement_unlocks_after_consumption() {
        let state = state().await;
        insert_definition(&state, "debt", "unpaid_amount", 50).await;
        sqlx::query(
            "INSERT INTO consumptions
             (id, user_id, machine_id, beverage_id, consumed_at, created_at)
             VALUES ('cup', 'user', 'main', 'coffee', 10, 10)",
        )
        .execute(&state.db)
        .await
        .unwrap();

        let unlocks = evaluate_after_consumption(&state, &user(), "cup", 10)
            .await
            .unwrap();
        assert_eq!(unlocks[0].id, "debt");
    }

    #[tokio::test]
    async fn payment_achievements_unlock_after_payment() {
        let state = state().await;
        insert_definition(&state, "first_payment", "payment_count", 1).await;
        insert_definition(&state, "big_payment", "single_payment_amount", 500).await;
        sqlx::query(
            "INSERT INTO payments
             (id, user_id, amount_cents, drink_count, period_end_ts, paid_at, created_at)
             VALUES ('payment', 'user', 500, 10, 20, 20, 20)",
        )
        .execute(&state.db)
        .await
        .unwrap();

        let unlocks = evaluate_after_payment(&state, "user", 500, 20)
            .await
            .unwrap();
        let ids = unlocks
            .into_iter()
            .map(|unlock| unlock.id)
            .collect::<Vec<_>>();
        assert!(ids.contains(&"first_payment".to_string()));
        assert!(ids.contains(&"big_payment".to_string()));
    }

    #[tokio::test]
    async fn guest_payment_does_not_unlock_achievements() {
        let state = state().await;
        sqlx::query("UPDATE users SET user_type = 'guest' WHERE id = 'user'")
            .execute(&state.db)
            .await
            .unwrap();
        insert_definition(&state, "first_payment", "payment_count", 1).await;
        sqlx::query(
            "INSERT INTO payments
             (id, user_id, amount_cents, drink_count, period_end_ts, paid_at, created_at)
             VALUES ('payment', 'user', 500, 10, 20, 20, 20)",
        )
        .execute(&state.db)
        .await
        .unwrap();

        let unlocks = evaluate_after_payment(&state, "user", 500, 20)
            .await
            .unwrap();
        assert!(unlocks.is_empty());
    }

    #[tokio::test]
    async fn beverage_specific_achievement_unlocks_after_matching_consumption() {
        let state = state().await;
        insert_definition(&state, "custom_eis", "beverage_count:eis", 1).await;
        insert_definition(&state, "espresso_only", "beverage_count:espresso_pads", 1).await;
        sqlx::query(
            "INSERT INTO consumptions
             (id, user_id, machine_id, beverage_id, consumed_at, created_at)
             VALUES ('ice', 'user', 'main', 'eis', 10, 10)",
        )
        .execute(&state.db)
        .await
        .unwrap();

        let unlocks = evaluate_after_consumption(&state, &user(), "ice", 10)
            .await
            .unwrap();
        let ids = unlocks
            .into_iter()
            .map(|unlock| unlock.id)
            .collect::<Vec<_>>();
        assert!(ids.contains(&"custom_eis".to_string()));
        assert!(!ids.contains(&"espresso_only".to_string()));
        assert!(!ids.contains(&"first_tea".to_string()));
    }

    #[tokio::test]
    async fn tea_achievements_unlock_after_tea_consumption() {
        let state = state().await;
        for index in 0..10 {
            sqlx::query(
                "INSERT INTO consumptions
                 (id, user_id, machine_id, beverage_id, consumed_at, created_at)
                 VALUES (?, 'user', 'main', 'tea', ?, ?)",
            )
            .bind(format!("tea-{index}"))
            .bind(index + 10)
            .bind(index + 10)
            .execute(&state.db)
            .await
            .unwrap();
        }

        let unlocks = evaluate_after_consumption(&state, &user(), "tea-9", 19)
            .await
            .unwrap();
        let ids = unlocks
            .into_iter()
            .map(|unlock| unlock.id)
            .collect::<Vec<_>>();
        assert!(ids.contains(&"first_tea".to_string()));
        assert!(ids.contains(&"tea_regular".to_string()));
        assert!(!ids.contains(&"tea_fan".to_string()));
    }

    #[tokio::test]
    async fn regular_menu_variety_requires_tea() {
        let state = state().await;
        insert_definition(&state, "full_menu_test", "regular_menu_variety", 4).await;
        for (id, beverage_id) in [
            ("coffee", "coffee"),
            ("pads", "espresso_pads"),
            ("ice", "eis"),
        ] {
            sqlx::query(
                "INSERT INTO consumptions
                 (id, user_id, machine_id, beverage_id, consumed_at, created_at)
                 VALUES (?, 'user', 'main', ?, 10, 10)",
            )
            .bind(id)
            .bind(beverage_id)
            .execute(&state.db)
            .await
            .unwrap();
        }
        let unlocks = evaluate_after_consumption(&state, &user(), "ice", 10)
            .await
            .unwrap();
        assert!(!unlocks.iter().any(|unlock| unlock.id == "full_menu_test"));

        sqlx::query(
            "INSERT INTO consumptions
             (id, user_id, machine_id, beverage_id, consumed_at, created_at)
             VALUES ('tea', 'user', 'main', 'tea', 20, 20)",
        )
        .execute(&state.db)
        .await
        .unwrap();
        let unlocks = evaluate_after_consumption(&state, &user(), "tea", 20)
            .await
            .unwrap();
        assert!(unlocks.iter().any(|unlock| unlock.id == "full_menu_test"));
    }

    #[tokio::test]
    async fn early_bird_uses_configured_timezone() {
        let mut state = state().await;
        state.config.timezone = "Europe/Berlin".into();
        insert_definition(&state, "early_bird", "early_bird", 1).await;
        let after_eight_local = Utc
            .with_ymd_and_hms(2026, 5, 5, 6, 30, 0)
            .unwrap()
            .timestamp();
        sqlx::query(
            "INSERT INTO consumptions
             (id, user_id, machine_id, beverage_id, consumed_at, created_at)
             VALUES ('after-eight', 'user', 'main', 'coffee', ?, ?)",
        )
        .bind(after_eight_local)
        .bind(after_eight_local)
        .execute(&state.db)
        .await
        .unwrap();

        let unlocks = evaluate_after_consumption(&state, &user(), "after-eight", after_eight_local)
            .await
            .unwrap();
        assert!(unlocks.is_empty());

        let before_eight_local = Utc
            .with_ymd_and_hms(2026, 5, 5, 5, 30, 0)
            .unwrap()
            .timestamp();
        sqlx::query(
            "INSERT INTO consumptions
             (id, user_id, machine_id, beverage_id, consumed_at, created_at)
             VALUES ('before-eight', 'user', 'main', 'coffee', ?, ?)",
        )
        .bind(before_eight_local)
        .bind(before_eight_local)
        .execute(&state.db)
        .await
        .unwrap();

        let unlocks =
            evaluate_after_consumption(&state, &user(), "before-eight", before_eight_local)
                .await
                .unwrap();
        assert!(unlocks.iter().any(|unlock| unlock.id == "early_bird"));
    }

    #[tokio::test]
    async fn hour_window_count_uses_configured_timezone() {
        let mut state = state().await;
        state.config.timezone = "Europe/Berlin".into();
        insert_definition(&state, "morning_regular", "hour_window_count:0:10", 2).await;
        let late_local = Utc
            .with_ymd_and_hms(2026, 5, 5, 8, 30, 0)
            .unwrap()
            .timestamp();
        let morning_one = Utc
            .with_ymd_and_hms(2026, 5, 5, 7, 30, 0)
            .unwrap()
            .timestamp();
        let morning_two = Utc
            .with_ymd_and_hms(2026, 5, 6, 6, 0, 0)
            .unwrap()
            .timestamp();
        insert_consumption(&state, "late-local", late_local).await;
        insert_consumption(&state, "morning-one", morning_one).await;

        let unlocks = evaluate_after_consumption(&state, &user(), "morning-one", morning_one)
            .await
            .unwrap();
        assert!(unlocks.is_empty());

        insert_consumption(&state, "morning-two", morning_two).await;
        let unlocks = evaluate_after_consumption(&state, &user(), "morning-two", morning_two)
            .await
            .unwrap();
        assert!(unlocks.iter().any(|unlock| unlock.id == "morning_regular"));
    }

    #[tokio::test]
    async fn hour_window_count_requires_current_consumption_inside_window() {
        let mut state = state().await;
        state.config.timezone = "Europe/Berlin".into();
        insert_definition(&state, "lunch_break", "hour_window_count:12:14", 5).await;
        for (id, timestamp) in [
            ("lunch-one", Utc.with_ymd_and_hms(2026, 5, 1, 10, 0, 0)),
            ("lunch-two", Utc.with_ymd_and_hms(2026, 5, 2, 10, 30, 0)),
            ("lunch-three", Utc.with_ymd_and_hms(2026, 5, 3, 11, 0, 0)),
            ("lunch-four", Utc.with_ymd_and_hms(2026, 5, 4, 11, 30, 0)),
            ("lunch-five", Utc.with_ymd_and_hms(2026, 5, 5, 10, 15, 0)),
        ] {
            insert_consumption(&state, id, timestamp.unwrap().timestamp()).await;
        }
        let after_lunch = Utc
            .with_ymd_and_hms(2026, 5, 6, 14, 0, 0)
            .unwrap()
            .timestamp();
        insert_consumption(&state, "after-lunch", after_lunch).await;

        let unlocks = evaluate_after_consumption(&state, &user(), "after-lunch", after_lunch)
            .await
            .unwrap();
        assert!(unlocks.is_empty());
    }

    #[tokio::test]
    async fn weekend_count_uses_configured_timezone() {
        let mut state = state().await;
        state.config.timezone = "Europe/Berlin".into();
        insert_definition(&state, "weekend_visit", "weekend_count", 1).await;
        let friday_utc_saturday_local = Utc
            .with_ymd_and_hms(2026, 5, 1, 22, 30, 0)
            .unwrap()
            .timestamp();
        insert_consumption(&state, "saturday-local", friday_utc_saturday_local).await;

        let unlocks = evaluate_after_consumption(
            &state,
            &user(),
            "saturday-local",
            friday_utc_saturday_local,
        )
        .await
        .unwrap();
        assert!(unlocks.iter().any(|unlock| unlock.id == "weekend_visit"));
    }

    #[tokio::test]
    async fn return_after_gap_days_compares_current_consumption_to_previous_one() {
        let state = state().await;
        insert_definition(&state, "welcome_back", "return_after_gap_days", 14).await;
        let first = Utc
            .with_ymd_and_hms(2026, 5, 1, 9, 0, 0)
            .unwrap()
            .timestamp();
        let too_soon = Utc
            .with_ymd_and_hms(2026, 5, 14, 9, 0, 0)
            .unwrap()
            .timestamp();
        let after_gap = Utc
            .with_ymd_and_hms(2026, 5, 28, 9, 0, 0)
            .unwrap()
            .timestamp();
        insert_consumption(&state, "first", first).await;
        insert_consumption(&state, "too-soon", too_soon).await;

        let unlocks = evaluate_after_consumption(&state, &user(), "too-soon", too_soon)
            .await
            .unwrap();
        assert!(unlocks.is_empty());

        insert_consumption(&state, "after-gap", after_gap).await;
        let unlocks = evaluate_after_consumption(&state, &user(), "after-gap", after_gap)
            .await
            .unwrap();
        assert!(unlocks.iter().any(|unlock| unlock.id == "welcome_back"));
    }

    #[tokio::test]
    async fn streak_achievement_treats_friday_to_monday_as_consecutive() {
        let state = state().await;
        insert_definition(&state, "lab_streak", "streak_days", 2).await;
        for (id, timestamp) in [
            (
                "friday",
                Utc.with_ymd_and_hms(2026, 5, 1, 12, 0, 0)
                    .unwrap()
                    .timestamp(),
            ),
            (
                "monday",
                Utc.with_ymd_and_hms(2026, 5, 4, 12, 0, 0)
                    .unwrap()
                    .timestamp(),
            ),
        ] {
            sqlx::query(
                "INSERT INTO consumptions
                 (id, user_id, machine_id, beverage_id, consumed_at, created_at)
                 VALUES (?, 'user', 'main', 'coffee', ?, ?)",
            )
            .bind(id)
            .bind(timestamp)
            .bind(timestamp)
            .execute(&state.db)
            .await
            .unwrap();
        }

        let unlocks = evaluate_after_consumption(&state, &user(), "monday", 10)
            .await
            .unwrap();
        assert!(unlocks.iter().any(|unlock| unlock.id == "lab_streak"));
    }
}
