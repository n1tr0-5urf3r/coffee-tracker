//! Trust-based "I cleaned the coffee machine" log. The first cleaning of each
//! local ISO week earns the cleaner a lootbox; later cleanings in the same week
//! are still recorded (and count towards achievements) but earn no box.

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use chrono::{Datelike, TimeZone, Utc};
use chrono_tz::Tz;
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    AppState,
    achievements::{AchievementEvent, evaluate_global},
    auth::CurrentUser,
    db::now_ts,
    error::{ApiError, ApiResult},
    notifications::notify_unlocks,
    stats::resolve_tz,
};

/// Minimum time between two cleanings by the same person.
const PER_USER_COOLDOWN_SECS: i64 = 12 * 60 * 60;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/cleanings", post(log_cleaning))
        .route("/cleanings/status", get(cleaning_status))
}

#[derive(Serialize, ToSchema)]
pub struct CleaningEntry {
    pub display_name: String,
    pub cleaned_at: i64,
    pub rewarded: bool,
}

#[derive(Serialize, ToSchema)]
pub struct CleaningStatus {
    /// Current local ISO week, e.g. `2026-W45`.
    pub week: String,
    /// True while nobody has claimed this week's cleaning lootbox.
    pub reward_available: bool,
    pub reward_claimed_by: Option<String>,
    pub reward_claimed_at: Option<i64>,
    /// Earliest time the current user may log another cleaning.
    pub next_allowed_at: Option<i64>,
    pub recent: Vec<CleaningEntry>,
}

#[derive(Serialize, ToSchema)]
pub struct CleaningResponse {
    /// Whether this cleaning earned the weekly lootbox.
    pub rewarded: bool,
    pub unlocked_achievements: Vec<AchievementEvent>,
    pub status: CleaningStatus,
}

pub fn week_key(timestamp: i64, tz: &Tz) -> String {
    let date = Utc
        .timestamp_opt(timestamp, 0)
        .single()
        .map(|dt| dt.with_timezone(tz).date_naive())
        .unwrap_or_else(|| Utc::now().with_timezone(tz).date_naive());
    let week = date.iso_week();
    format!("{:04}-W{:02}", week.year(), week.week())
}

#[utoipa::path(
    get,
    path = "/api/v1/cleanings/status",
    responses((status = 200, body = CleaningStatus))
)]
pub(crate) async fn cleaning_status(
    State(state): State<AppState>,
    user: CurrentUser,
) -> ApiResult<Json<CleaningStatus>> {
    Ok(Json(load_status(&state, &user.id, now_ts()).await?))
}

#[utoipa::path(
    post,
    path = "/api/v1/cleanings",
    responses(
        (status = 201, body = CleaningResponse),
        (status = 409, description = "Cleaned too recently")
    )
)]
pub(crate) async fn log_cleaning(
    State(state): State<AppState>,
    user: CurrentUser,
) -> ApiResult<(StatusCode, Json<CleaningResponse>)> {
    let now = now_ts();
    let tz = resolve_tz(&state.config.timezone);
    let week = week_key(now, &tz);
    let id = Uuid::new_v4().to_string();
    // Guests can log cleanings, but only members can win the weekly crate.
    let can_win = user.user_type == "member";

    let mut tx = state.db.begin().await?;
    // One statement: the cooldown check and the weekly-reward check happen
    // under the same write lock as the insert, so concurrent requests can't
    // both pass them.
    let inserted = sqlx::query(
        "INSERT INTO machine_cleanings (id, user_id, cleaned_at, week_key, rewarded)
         SELECT ?, ?, ?, ?,
                ? AND NOT EXISTS (
                    SELECT 1 FROM machine_cleanings WHERE week_key = ? AND rewarded = 1
                )
         WHERE NOT EXISTS (
             SELECT 1 FROM machine_cleanings WHERE user_id = ? AND cleaned_at > ?
         )",
    )
    .bind(&id)
    .bind(&user.id)
    .bind(now)
    .bind(&week)
    .bind(can_win)
    .bind(&week)
    .bind(&user.id)
    .bind(now - PER_USER_COOLDOWN_SECS)
    .execute(&mut *tx)
    .await?;
    if inserted.rows_affected() == 0 {
        return Err(ApiError::Conflict(
            "you already logged a cleaning in the last 12 hours".into(),
        ));
    }
    let (rewarded,): (bool,) =
        sqlx::query_as("SELECT rewarded FROM machine_cleanings WHERE id = ?")
            .bind(&id)
            .fetch_one(&mut *tx)
            .await?;
    if rewarded {
        // Same transaction: the week can't be marked as rewarded without the crate.
        sqlx::query(
            "INSERT OR IGNORE INTO lootbox_grants (id, user_id, source, source_ref, granted_at)
             VALUES (?, ?, 'cleaning', ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&user.id)
        .bind(&week)
        .bind(now)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    let unlocked_achievements = evaluate_global(&state, &user.id, now).await?;
    notify_unlocks(&state, &user, &unlocked_achievements).await;

    Ok((
        StatusCode::CREATED,
        Json(CleaningResponse {
            rewarded,
            unlocked_achievements,
            status: load_status(&state, &user.id, now).await?,
        }),
    ))
}

async fn next_allowed_at(state: &AppState, user_id: &str) -> ApiResult<Option<i64>> {
    let last: Option<(i64,)> = sqlx::query_as(
        "SELECT cleaned_at FROM machine_cleanings WHERE user_id = ? ORDER BY cleaned_at DESC LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?;
    Ok(last.map(|(cleaned_at,)| cleaned_at + PER_USER_COOLDOWN_SECS))
}

async fn load_status(state: &AppState, user_id: &str, now: i64) -> ApiResult<CleaningStatus> {
    let tz = resolve_tz(&state.config.timezone);
    let week = week_key(now, &tz);
    // Names only for people who share them on the leaderboard.
    let claimed: Option<(String, i64)> = sqlx::query_as(
        "SELECT CASE WHEN u.user_type = 'member' AND u.leaderboard_opt_in = 1
                     THEN u.display_name ELSE 'A colleague' END,
                mc.cleaned_at
         FROM machine_cleanings mc
         JOIN users u ON u.id = mc.user_id
         WHERE mc.week_key = ? AND mc.rewarded = 1",
    )
    .bind(&week)
    .fetch_optional(&state.db)
    .await?;
    let recent = sqlx::query_as::<_, (String, i64, bool)>(
        "SELECT CASE WHEN u.user_type = 'member' AND u.leaderboard_opt_in = 1
                     THEN u.display_name ELSE 'A colleague' END,
                mc.cleaned_at, mc.rewarded
         FROM machine_cleanings mc
         JOIN users u ON u.id = mc.user_id
         ORDER BY mc.cleaned_at DESC
         LIMIT 5",
    )
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|(display_name, cleaned_at, rewarded)| CleaningEntry {
        display_name,
        cleaned_at,
        rewarded,
    })
    .collect();
    let next_allowed_at = next_allowed_at(state, user_id)
        .await?
        .filter(|next_allowed_at| *next_allowed_at > now);

    Ok(CleaningStatus {
        week,
        reward_available: claimed.is_none(),
        reward_claimed_by: claimed.as_ref().map(|(name, _)| name.clone()),
        reward_claimed_at: claimed.map(|(_, cleaned_at)| cleaned_at),
        next_allowed_at,
        recent,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn week_key_uses_iso_weeks_in_local_time() {
        let tz: Tz = "Europe/Berlin".parse().unwrap();
        // Sunday 2026-11-08 23:30 Berlin is still ISO week 45 ...
        let sunday_late = tz
            .with_ymd_and_hms(2026, 11, 8, 23, 30, 0)
            .unwrap()
            .timestamp();
        assert_eq!(week_key(sunday_late, &tz), "2026-W45");
        // ... and Monday 00:10 starts week 46, although it is still Sunday in UTC.
        let monday_early = tz
            .with_ymd_and_hms(2026, 11, 9, 0, 10, 0)
            .unwrap()
            .timestamp();
        assert_eq!(week_key(monday_early, &tz), "2026-W46");
        // ISO year differs from the calendar year around New Year.
        let new_year = tz
            .with_ymd_and_hms(2027, 1, 1, 12, 0, 0)
            .unwrap()
            .timestamp();
        assert_eq!(week_key(new_year, &tz), "2026-W53");
    }
}

#[cfg(test)]
mod flow_tests {
    use axum::extract::State;

    use super::*;
    use crate::cosmetics::{
        load_locker,
        test_support::{member, state_with_users},
    };

    #[tokio::test]
    async fn only_the_first_cleaning_of_a_week_earns_a_lootbox() {
        let state = state_with_users(&["alice", "bob"]).await;

        let (_, first) = log_cleaning(State(state.clone()), member("alice"))
            .await
            .unwrap();
        assert!(first.rewarded);
        assert!(!first.status.reward_available);
        assert_eq!(first.status.reward_claimed_by.as_deref(), Some("ALICE"));
        assert!(
            first
                .unlocked_achievements
                .iter()
                .any(|achievement| achievement.id == "clean_machine")
        );

        let (_, second) = log_cleaning(State(state.clone()), member("bob"))
            .await
            .unwrap();
        assert!(!second.rewarded);
        assert_eq!(second.status.recent.len(), 2);

        let error = log_cleaning(State(state.clone()), member("alice"))
            .await
            .err()
            .unwrap();
        assert!(matches!(error, ApiError::Conflict(_)));

        assert_eq!(
            load_locker(&state, "alice").await.unwrap().sources.cleaning,
            1
        );
        assert_eq!(
            load_locker(&state, "bob").await.unwrap().sources.cleaning,
            0
        );
    }

    #[tokio::test]
    async fn guests_can_clean_but_not_win_and_opted_out_names_stay_hidden() {
        let state = state_with_users(&["guest", "shy", "alice"]).await;
        sqlx::query("UPDATE users SET user_type = 'guest' WHERE id = 'guest'")
            .execute(&state.db)
            .await
            .unwrap();
        sqlx::query("UPDATE users SET leaderboard_opt_in = 0 WHERE id = 'shy'")
            .execute(&state.db)
            .await
            .unwrap();
        let mut guest = member("guest");
        guest.user_type = "guest".into();

        let (_, by_guest) = log_cleaning(State(state.clone()), guest).await.unwrap();
        assert!(!by_guest.rewarded);
        assert!(by_guest.status.reward_available);

        let (_, by_shy) = log_cleaning(State(state.clone()), member("shy"))
            .await
            .unwrap();
        assert!(by_shy.rewarded);
        assert_eq!(
            by_shy.status.reward_claimed_by.as_deref(),
            Some("A colleague")
        );
        assert!(
            by_shy
                .status
                .recent
                .iter()
                .all(|entry| entry.display_name == "A colleague")
        );
    }

    #[tokio::test]
    async fn the_weekly_reward_is_unique_per_week_in_the_database() {
        let state = state_with_users(&["alice", "bob"]).await;
        for user in ["alice", "bob"] {
            let result = sqlx::query(
                "INSERT INTO machine_cleanings (id, user_id, cleaned_at, week_key, rewarded)
                 VALUES (?, ?, 1, '2026-W45', 1)",
            )
            .bind(Uuid::new_v4().to_string())
            .bind(user)
            .execute(&state.db)
            .await;
            assert_eq!(result.is_ok(), user == "alice");
        }
    }
}
