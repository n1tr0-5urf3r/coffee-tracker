//! Trust-based chores board for the coffee machine (cleaning, emptying the
//! grounds, refilling water, descaling, "Tassen machen": taking the used cups
//! to the dishwasher and bringing clean ones). Every chore has a reward period: a
//! local calendar day for the everyday ones (grounds, water), a local ISO week
//! for the rest. The first member to do a chore in its period earns a lootbox;
//! later entries are still recorded (and count towards achievements) but earn
//! no box. Guests can log chores but never win the crate.

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
};
use chrono::{Datelike, NaiveDate, TimeZone, Utc};
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

/// Minimum time between two entries of the same chore by the same person.
const PER_USER_COOLDOWN_SECS: i64 = 12 * 60 * 60;

/// How often a chore's crate can be won.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChorePeriod {
    Day,
    Week,
}

impl ChorePeriod {
    fn as_str(self) -> &'static str {
        match self {
            ChorePeriod::Day => "day",
            ChorePeriod::Week => "week",
        }
    }

    /// Key of the period containing `timestamp`: `2026-11-10` for a day,
    /// `2026-W46` for a week. Stored in `machine_cleanings.week_key`.
    pub fn key(self, timestamp: i64, tz: &Tz) -> String {
        match self {
            ChorePeriod::Day => local_date(timestamp, tz).format("%Y-%m-%d").to_string(),
            ChorePeriod::Week => week_key(timestamp, tz),
        }
    }
}

/// (id, label, reward period) of every chore, in display order.
pub const CHORES: [(&str, &str, ChorePeriod); 5] = [
    ("clean", "Clean the machine", ChorePeriod::Week),
    ("grounds", "Empty the grounds container", ChorePeriod::Day),
    ("water", "Refill the water tank", ChorePeriod::Day),
    ("descale", "Descale the machine", ChorePeriod::Week),
    // Take the used cups to the dishwasher and bring clean ones back.
    ("cups", "Tassen machen", ChorePeriod::Week),
];

pub fn router() -> Router<AppState> {
    Router::new()
        // `/cleanings` predates the chores board and stays as an alias for "clean".
        .route("/cleanings", post(log_cleaning))
        .route("/cleanings/status", get(cleaning_status))
        .route("/chores", get(chores_status))
        .route("/chores/{chore}", post(log_chore))
}

#[derive(Serialize, ToSchema)]
pub struct CleaningEntry {
    pub chore: String,
    pub display_name: String,
    pub cleaned_at: i64,
    pub rewarded: bool,
}

/// State of one chore for its current reward period.
#[derive(Serialize, ToSchema)]
pub struct CleaningStatus {
    pub chore: String,
    pub label: String,
    /// Current local ISO week, e.g. `2026-W45`.
    pub week: String,
    /// Reward period of the chore: `day` or `week`.
    pub period: String,
    /// Current period, e.g. `2026-11-10` (day) or `2026-W45` (week).
    pub period_key: String,
    /// True while nobody has claimed the current period's crate for the chore.
    pub reward_available: bool,
    pub reward_claimed_by: Option<String>,
    pub reward_claimed_at: Option<i64>,
    /// Earliest time the current user may log this chore again.
    pub next_allowed_at: Option<i64>,
    /// Latest entries of this chore.
    pub recent: Vec<CleaningEntry>,
}

#[derive(Serialize, ToSchema)]
pub struct CleaningResponse {
    /// Whether this entry earned the chore's lootbox for the period.
    pub rewarded: bool,
    pub unlocked_achievements: Vec<AchievementEvent>,
    pub status: CleaningStatus,
}

#[derive(Serialize, ToSchema)]
pub struct ChoresResponse {
    pub week: String,
    pub chores: Vec<CleaningStatus>,
}

#[derive(Serialize, ToSchema)]
pub struct ChoreLogResponse {
    pub chore: String,
    /// Whether this entry earned the chore's lootbox for the period.
    pub rewarded: bool,
    pub unlocked_achievements: Vec<AchievementEvent>,
    pub board: ChoresResponse,
}

fn local_date(timestamp: i64, tz: &Tz) -> NaiveDate {
    Utc.timestamp_opt(timestamp, 0)
        .single()
        .map(|dt| dt.with_timezone(tz).date_naive())
        .unwrap_or_else(|| Utc::now().with_timezone(tz).date_naive())
}

pub fn week_key(timestamp: i64, tz: &Tz) -> String {
    let week = local_date(timestamp, tz).iso_week();
    format!("{:04}-W{:02}", week.year(), week.week())
}

/// `lootbox_grants.source_ref` of a chore crate. Cleaning keeps the plain
/// week key it had before the chores board existed.
pub fn chore_grant_ref(chore: &str, period_key: &str) -> String {
    if chore == "clean" {
        period_key.to_string()
    } else {
        format!("{chore}:{period_key}")
    }
}

fn chore_info(chore: &str) -> ApiResult<(&'static str, ChorePeriod)> {
    CHORES
        .iter()
        .find(|(id, _, _)| *id == chore)
        .map(|(_, label, period)| (*label, *period))
        .ok_or(ApiError::NotFound)
}

#[utoipa::path(
    get,
    path = "/api/v1/chores",
    responses((status = 200, body = ChoresResponse))
)]
pub(crate) async fn chores_status(
    State(state): State<AppState>,
    user: CurrentUser,
) -> ApiResult<Json<ChoresResponse>> {
    Ok(Json(load_board(&state, &user.id, now_ts()).await?))
}

#[utoipa::path(
    post,
    path = "/api/v1/chores/{chore}",
    params(("chore" = String, Path, description = "clean, grounds, water, descale or cups")),
    responses(
        (status = 201, body = ChoreLogResponse),
        (status = 404, description = "Unknown chore"),
        (status = 409, description = "Logged this chore too recently")
    )
)]
pub(crate) async fn log_chore(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(chore): Path<String>,
) -> ApiResult<(StatusCode, Json<ChoreLogResponse>)> {
    let now = now_ts();
    let (rewarded, unlocked_achievements) = record_chore(&state, &user, &chore, now).await?;
    Ok((
        StatusCode::CREATED,
        Json(ChoreLogResponse {
            chore,
            rewarded,
            unlocked_achievements,
            board: load_board(&state, &user.id, now).await?,
        }),
    ))
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
    Ok(Json(
        load_status(&state, &user.id, "clean", now_ts()).await?,
    ))
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
    let (rewarded, unlocked_achievements) = record_chore(&state, &user, "clean", now).await?;
    Ok((
        StatusCode::CREATED,
        Json(CleaningResponse {
            rewarded,
            unlocked_achievements,
            status: load_status(&state, &user.id, "clean", now).await?,
        }),
    ))
}

/// Records one chore entry and, if it is the first member entry of the chore's
/// period, grants that period's crate. Returns whether it was rewarded and new
/// achievements.
async fn record_chore(
    state: &AppState,
    user: &CurrentUser,
    chore: &str,
    now: i64,
) -> ApiResult<(bool, Vec<AchievementEvent>)> {
    let (_, period) = chore_info(chore)?;
    let tz = resolve_tz(&state.config.timezone);
    let period_key = period.key(now, &tz);
    let id = Uuid::new_v4().to_string();
    // Guests can log chores, but only members can win the crate.
    let can_win = user.user_type == "member";

    let mut tx = state.db.begin().await?;
    // One statement: the cooldown check and the reward check happen under the
    // same write lock as the insert, so concurrent requests can't both pass
    // them. A person wins at most one chore crate per period (one weekly and
    // one daily crate), so one early bird can't sweep the whole board.
    let inserted = sqlx::query(
        "INSERT INTO machine_cleanings (id, user_id, chore, cleaned_at, week_key, rewarded)
         SELECT ?, ?, ?, ?, ?,
                ? AND NOT EXISTS (
                    SELECT 1 FROM machine_cleanings
                    WHERE chore = ? AND week_key = ? AND rewarded = 1
                ) AND NOT EXISTS (
                    SELECT 1 FROM machine_cleanings
                    WHERE user_id = ? AND week_key = ? AND rewarded = 1
                )
         WHERE NOT EXISTS (
             SELECT 1 FROM machine_cleanings
             WHERE user_id = ? AND chore = ? AND cleaned_at > ?
         )",
    )
    .bind(&id)
    .bind(&user.id)
    .bind(chore)
    .bind(now)
    .bind(&period_key)
    .bind(can_win)
    .bind(chore)
    .bind(&period_key)
    .bind(&user.id)
    .bind(&period_key)
    .bind(&user.id)
    .bind(chore)
    .bind(now - PER_USER_COOLDOWN_SECS)
    .execute(&mut *tx)
    .await?;
    if inserted.rows_affected() == 0 {
        return Err(ApiError::Conflict(
            "you already logged this chore in the last 12 hours".into(),
        ));
    }
    let (rewarded,): (bool,) =
        sqlx::query_as("SELECT rewarded FROM machine_cleanings WHERE id = ?")
            .bind(&id)
            .fetch_one(&mut *tx)
            .await?;
    if rewarded {
        // Same transaction: the period can't be marked as rewarded without the crate.
        sqlx::query(
            "INSERT OR IGNORE INTO lootbox_grants (id, user_id, source, source_ref, granted_at)
             VALUES (?, ?, 'cleaning', ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&user.id)
        .bind(chore_grant_ref(chore, &period_key))
        .bind(now)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    let unlocked_achievements = evaluate_global(state, &user.id, now).await?;
    notify_unlocks(state, user, &unlocked_achievements).await;
    Ok((rewarded, unlocked_achievements))
}

async fn next_allowed_at(state: &AppState, user_id: &str, chore: &str) -> ApiResult<Option<i64>> {
    let last: Option<(i64,)> = sqlx::query_as(
        "SELECT cleaned_at FROM machine_cleanings
         WHERE user_id = ? AND chore = ?
         ORDER BY cleaned_at DESC LIMIT 1",
    )
    .bind(user_id)
    .bind(chore)
    .fetch_optional(&state.db)
    .await?;
    Ok(last.map(|(cleaned_at,)| cleaned_at + PER_USER_COOLDOWN_SECS))
}

async fn load_board(state: &AppState, user_id: &str, now: i64) -> ApiResult<ChoresResponse> {
    let tz = resolve_tz(&state.config.timezone);
    let mut chores = Vec::with_capacity(CHORES.len());
    for (chore, _, _) in CHORES {
        chores.push(load_status(state, user_id, chore, now).await?);
    }
    Ok(ChoresResponse {
        week: week_key(now, &tz),
        chores,
    })
}

async fn load_status(
    state: &AppState,
    user_id: &str,
    chore: &str,
    now: i64,
) -> ApiResult<CleaningStatus> {
    let (label, period) = chore_info(chore)?;
    let tz = resolve_tz(&state.config.timezone);
    let period_key = period.key(now, &tz);
    // Names only for people who share them on the leaderboard.
    let claimed: Option<(String, i64)> = sqlx::query_as(
        "SELECT CASE WHEN u.user_type = 'member' AND u.leaderboard_opt_in = 1
                     THEN u.display_name ELSE 'A colleague' END,
                mc.cleaned_at
         FROM machine_cleanings mc
         JOIN users u ON u.id = mc.user_id
         WHERE mc.chore = ? AND mc.week_key = ? AND mc.rewarded = 1",
    )
    .bind(chore)
    .bind(&period_key)
    .fetch_optional(&state.db)
    .await?;
    let recent = sqlx::query_as::<_, (String, i64, bool)>(
        "SELECT CASE WHEN u.user_type = 'member' AND u.leaderboard_opt_in = 1
                     THEN u.display_name ELSE 'A colleague' END,
                mc.cleaned_at, mc.rewarded
         FROM machine_cleanings mc
         JOIN users u ON u.id = mc.user_id
         WHERE mc.chore = ?
         ORDER BY mc.cleaned_at DESC
         LIMIT 5",
    )
    .bind(chore)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|(display_name, cleaned_at, rewarded)| CleaningEntry {
        chore: chore.to_string(),
        display_name,
        cleaned_at,
        rewarded,
    })
    .collect();
    let next_allowed_at = next_allowed_at(state, user_id, chore)
        .await?
        .filter(|next_allowed_at| *next_allowed_at > now);

    Ok(CleaningStatus {
        chore: chore.to_string(),
        label: label.to_string(),
        week: week_key(now, &tz),
        period: period.as_str().to_string(),
        period_key,
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

    #[test]
    fn day_keys_follow_the_local_calendar_day() {
        let tz: Tz = "Europe/Berlin".parse().unwrap();
        // 00:10 Berlin is already the 10th although it is still the 9th in UTC.
        let just_after_midnight = tz
            .with_ymd_and_hms(2026, 11, 10, 0, 10, 0)
            .unwrap()
            .timestamp();
        assert_eq!(ChorePeriod::Day.key(just_after_midnight, &tz), "2026-11-10");
        assert_eq!(ChorePeriod::Week.key(just_after_midnight, &tz), "2026-W46");
    }

    #[test]
    fn cleaning_keeps_its_old_grant_ref() {
        assert_eq!(chore_grant_ref("clean", "2026-W45"), "2026-W45");
        assert_eq!(chore_grant_ref("descale", "2026-W45"), "descale:2026-W45");
        assert_eq!(chore_grant_ref("water", "2026-11-10"), "water:2026-11-10");
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
    async fn every_chore_has_its_own_crate_and_cooldown() {
        let state = state_with_users(&["alice", "bob", "carol"]).await;
        let log = |user: &str, chore: &str| {
            log_chore(State(state.clone()), member(user), Path(chore.into()))
        };

        let (_, clean) = log("alice", "clean").await.unwrap();
        assert!(clean.rewarded);
        // Same person, different chore: no cooldown, and grounds is a daily
        // chore, so it has its own crate next to the weekly one.
        assert!(log("alice", "grounds").await.unwrap().1.rewarded);
        // But one daily crate per person and day, so water stays up for grabs.
        assert!(!log("alice", "water").await.unwrap().1.rewarded);
        assert!(log("bob", "water").await.unwrap().1.rewarded);
        // Bob has no weekly crate yet.
        let (_, descale) = log("bob", "descale").await.unwrap();
        assert!(descale.rewarded);
        // Alice and Bob each have their weekly crate, so cups goes to Carol.
        assert!(!log("alice", "cups").await.unwrap().1.rewarded);
        let (_, cups) = log("carol", "cups").await.unwrap();
        assert!(cups.rewarded);
        let board = &cups.board;
        assert_eq!(board.chores.len(), CHORES.len());
        assert!(board.chores.iter().all(|c| !c.reward_available));
        let periods: Vec<_> = board.chores.iter().map(|c| c.period.as_str()).collect();
        assert_eq!(periods, ["week", "day", "day", "week", "week"]);

        // Cooldown is per chore.
        assert!(matches!(
            log("alice", "grounds").await.err().unwrap(),
            ApiError::Conflict(_)
        ));
        assert!(matches!(
            log("alice", "teleport").await.err().unwrap(),
            ApiError::NotFound
        ));

        assert_eq!(
            load_locker(&state, "alice").await.unwrap().sources.cleaning,
            2
        );
        assert_eq!(
            load_locker(&state, "bob").await.unwrap().sources.cleaning,
            2
        );
    }

    #[tokio::test]
    async fn daily_chores_offer_a_new_crate_every_day() {
        let state = state_with_users(&["alice"]).await;
        let tz = resolve_tz(&state.config.timezone);
        let monday = tz
            .with_ymd_and_hms(2026, 11, 9, 9, 0, 0)
            .unwrap()
            .timestamp();
        let tuesday = monday + 24 * 60 * 60;

        assert!(
            record_chore(&state, &member("alice"), "grounds", monday)
                .await
                .unwrap()
                .0
        );
        assert!(
            record_chore(&state, &member("alice"), "clean", monday)
                .await
                .unwrap()
                .0
        );
        let board = load_board(&state, "alice", tuesday).await.unwrap();
        let status = |chore: &str| board.chores.iter().find(|c| c.chore == chore).unwrap();
        assert!(status("grounds").reward_available);
        assert_eq!(status("grounds").period_key, "2026-11-10");
        // The weekly crate stays claimed for the rest of the week.
        assert!(!status("clean").reward_available);

        assert!(
            record_chore(&state, &member("alice"), "grounds", tuesday)
                .await
                .unwrap()
                .0
        );
        assert!(
            !record_chore(&state, &member("alice"), "clean", tuesday)
                .await
                .unwrap()
                .0
        );
        assert_eq!(
            load_locker(&state, "alice").await.unwrap().sources.cleaning,
            3
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
    async fn the_weekly_reward_is_unique_per_chore_and_week_in_the_database() {
        let state = state_with_users(&["alice", "bob"]).await;
        let insert = |user: &'static str, chore: &'static str| {
            sqlx::query(
                "INSERT INTO machine_cleanings (id, user_id, chore, cleaned_at, week_key, rewarded)
                 VALUES (?, ?, ?, 1, '2026-W45', 1)",
            )
            .bind(Uuid::new_v4().to_string())
            .bind(user)
            .bind(chore)
            .execute(&state.db)
        };
        assert!(insert("alice", "clean").await.is_ok());
        assert!(insert("bob", "clean").await.is_err());
        assert!(insert("bob", "water").await.is_ok());
    }
}
