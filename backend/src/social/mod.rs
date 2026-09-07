use std::collections::{HashMap, HashSet};

use axum::{Json, Router, extract::State, routing::get};
use chrono::{Datelike, Duration, NaiveDate, TimeZone, Timelike, Utc, Weekday};
use chrono_tz::Tz;
use serde::Serialize;
use utoipa::ToSchema;

use crate::{
    AppState,
    auth::CurrentUser,
    db::now_ts,
    error::{ApiError, ApiResult},
    stats::resolve_tz,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/leaderboard", get(leaderboard))
        .route("/coffee-twins", get(coffee_twins))
}

#[derive(Serialize, ToSchema)]
pub struct LeaderboardEntry {
    pub rank: i64,
    pub user_id: String,
    pub display_name: String,
    pub coffee_count: i64,
}

#[derive(Serialize, ToSchema)]
pub struct LeaderboardResponse {
    pub entries: Vec<LeaderboardEntry>,
    pub streak_entries: Vec<StreakLeaderboardEntry>,
    pub achievement_entries: Vec<AchievementLeaderboardEntry>,
    pub rare_achievements: Vec<RareAchievement>,
    pub recent_unlocks: Vec<RecentAchievementUnlock>,
    pub summary: LeaderboardSummary,
}

#[derive(Serialize, ToSchema)]
pub struct StreakLeaderboardEntry {
    pub rank: i64,
    pub user_id: String,
    pub display_name: String,
    pub current_streak_days: i64,
    pub longest_streak_days: i64,
    pub drink_count: i64,
}

#[derive(Serialize, ToSchema)]
pub struct AchievementLeaderboardEntry {
    pub rank: i64,
    pub user_id: String,
    pub display_name: String,
    pub unlocked_count: i64,
    pub latest_unlocked_at: Option<i64>,
    pub unlocks: Vec<AchievementUnlockSummary>,
}

#[derive(Serialize, ToSchema)]
pub struct AchievementUnlockSummary {
    pub achievement_id: String,
    pub name: String,
    pub unlocked_at: i64,
}

#[derive(Serialize, ToSchema)]
pub struct RareAchievement {
    pub achievement_id: String,
    pub name: String,
    pub unlock_count: i64,
}

#[derive(Serialize, ToSchema)]
pub struct RecentAchievementUnlock {
    pub user_id: String,
    pub display_name: String,
    pub achievement_id: String,
    pub achievement_name: String,
    pub unlocked_at: i64,
}

#[derive(Serialize, ToSchema)]
pub struct LeaderboardSummary {
    pub visible_participants: i64,
    pub active_streaks: i64,
    pub total_public_unlocks: i64,
    pub total_active_achievements: i64,
}

#[derive(Serialize, ToSchema)]
pub struct CoffeeTwin {
    pub user_id: String,
    pub display_name: String,
    pub similarity: f64,
}

#[derive(Serialize, ToSchema)]
pub struct CoffeeTwinsResponse {
    pub twins: Vec<CoffeeTwin>,
}

#[utoipa::path(get, path = "/api/v1/leaderboard", responses((status = 200, body = LeaderboardResponse)))]
pub(crate) async fn leaderboard(
    State(state): State<AppState>,
) -> ApiResult<Json<LeaderboardResponse>> {
    let tz = resolve_tz(&state.config.timezone);
    let today = Utc
        .timestamp_opt(now_ts(), 0)
        .single()
        .map(|dt| dt.with_timezone(&tz).date_naive())
        .unwrap_or_else(|| Utc::now().with_timezone(&tz).date_naive());
    let rows = sqlx::query_as::<_, LeaderboardRow>(
        "SELECT u.id AS user_id, u.display_name, COUNT(c.id) AS coffee_count
         FROM users u
         JOIN consumptions c ON c.user_id = u.id AND c.beverage_id = 'coffee'
         WHERE u.user_type = 'member' AND u.leaderboard_opt_in = 1
         GROUP BY u.id, u.display_name
         ORDER BY coffee_count DESC, u.display_name ASC
         LIMIT 50",
    )
    .fetch_all(&state.db)
    .await?;
    let entries = rows
        .into_iter()
        .enumerate()
        .map(|(idx, row)| LeaderboardEntry {
            rank: idx as i64 + 1,
            user_id: row.user_id,
            display_name: row.display_name,
            coffee_count: row.coffee_count,
        })
        .collect();

    let (visible_participants,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*)
         FROM users
         WHERE user_type = 'member' AND leaderboard_opt_in = 1",
    )
    .fetch_one(&state.db)
    .await?;
    let users = sqlx::query_as::<_, VisibleUserRow>(
        "SELECT id AS user_id, display_name
         FROM users
         WHERE user_type = 'member' AND leaderboard_opt_in = 1",
    )
    .fetch_all(&state.db)
    .await?;
    let consumption_rows = sqlx::query_as::<_, UserConsumptionRow>(
        "SELECT u.id AS user_id, c.consumed_at
         FROM users u
         JOIN consumptions c ON c.user_id = u.id
         WHERE u.user_type = 'member' AND u.leaderboard_opt_in = 1",
    )
    .fetch_all(&state.db)
    .await?;
    let mut streak_entries = streak_entries(users, consumption_rows, today, &tz);
    let active_streaks = streak_entries
        .iter()
        .filter(|entry| entry.current_streak_days > 0)
        .count() as i64;
    streak_entries.truncate(10);

    let achievement_rows = sqlx::query_as::<_, AchievementLeaderboardRow>(
        "SELECT u.id AS user_id,
                u.display_name,
                COUNT(au.id) AS unlocked_count,
                MAX(au.unlocked_at) AS latest_unlocked_at
         FROM users u
         JOIN achievement_unlocks au ON au.user_id = u.id
         WHERE u.user_type = 'member'
           AND u.leaderboard_opt_in = 1
           AND u.achievement_sharing_opt_out = 0
         GROUP BY u.id, u.display_name
         ORDER BY unlocked_count DESC, latest_unlocked_at DESC, u.display_name ASC
         LIMIT 10",
    )
    .fetch_all(&state.db)
    .await?;
    let unlock_rows = sqlx::query_as::<_, AchievementUnlockSummaryRow>(
        "SELECT u.id AS user_id,
                d.id AS achievement_id,
                d.name,
                au.unlocked_at
         FROM achievement_unlocks au
         JOIN users u ON u.id = au.user_id
         JOIN achievement_definitions d ON d.id = au.achievement_id
         WHERE d.active = 1
           AND u.user_type = 'member'
           AND u.leaderboard_opt_in = 1
           AND u.achievement_sharing_opt_out = 0
         ORDER BY au.unlocked_at DESC, d.name ASC",
    )
    .fetch_all(&state.db)
    .await?;
    let mut unlocks_by_user = HashMap::<String, Vec<AchievementUnlockSummary>>::new();
    for row in unlock_rows {
        unlocks_by_user
            .entry(row.user_id)
            .or_default()
            .push(AchievementUnlockSummary {
                achievement_id: row.achievement_id,
                name: row.name,
                unlocked_at: row.unlocked_at,
            });
    }
    let achievement_entries = achievement_rows
        .into_iter()
        .enumerate()
        .map(|(idx, row)| {
            let unlocks = unlocks_by_user.remove(&row.user_id).unwrap_or_default();
            AchievementLeaderboardEntry {
                rank: idx as i64 + 1,
                user_id: row.user_id,
                display_name: row.display_name,
                unlocked_count: row.unlocked_count,
                latest_unlocked_at: row.latest_unlocked_at,
                unlocks,
            }
        })
        .collect::<Vec<_>>();

    let rare_achievements = sqlx::query_as::<_, RareAchievementRow>(
        "SELECT d.id AS achievement_id, d.name, COUNT(au.id) AS unlock_count
         FROM achievement_definitions d
         JOIN achievement_unlocks au ON au.achievement_id = d.id
         JOIN users u ON u.id = au.user_id
         WHERE d.active = 1
           AND u.user_type = 'member'
           AND u.leaderboard_opt_in = 1
           AND u.achievement_sharing_opt_out = 0
         GROUP BY d.id, d.name
         HAVING COUNT(au.id) > 0
         ORDER BY unlock_count ASC, d.name ASC
         LIMIT 5",
    )
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|row| RareAchievement {
        achievement_id: row.achievement_id,
        name: row.name,
        unlock_count: row.unlock_count,
    })
    .collect();

    let recent_unlocks = sqlx::query_as::<_, RecentAchievementUnlockRow>(
        "SELECT u.id AS user_id,
                u.display_name,
                d.id AS achievement_id,
                d.name AS achievement_name,
                au.unlocked_at
         FROM achievement_unlocks au
         JOIN users u ON u.id = au.user_id
         JOIN achievement_definitions d ON d.id = au.achievement_id
         WHERE d.active = 1
           AND u.user_type = 'member'
           AND u.leaderboard_opt_in = 1
           AND u.achievement_sharing_opt_out = 0
         ORDER BY au.unlocked_at DESC, u.display_name ASC
         LIMIT 8",
    )
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|row| RecentAchievementUnlock {
        user_id: row.user_id,
        display_name: row.display_name,
        achievement_id: row.achievement_id,
        achievement_name: row.achievement_name,
        unlocked_at: row.unlocked_at,
    })
    .collect();

    let (total_public_unlocks,): (i64,) = sqlx::query_as(
        "SELECT COUNT(au.id)
         FROM achievement_unlocks au
         JOIN users u ON u.id = au.user_id
         JOIN achievement_definitions d ON d.id = au.achievement_id
         WHERE d.active = 1
           AND u.user_type = 'member'
           AND u.leaderboard_opt_in = 1
           AND u.achievement_sharing_opt_out = 0",
    )
    .fetch_one(&state.db)
    .await?;
    let (total_active_achievements,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM achievement_definitions WHERE active = 1")
            .fetch_one(&state.db)
            .await?;

    Ok(Json(LeaderboardResponse {
        entries,
        streak_entries,
        achievement_entries,
        rare_achievements,
        recent_unlocks,
        summary: LeaderboardSummary {
            visible_participants,
            active_streaks,
            total_public_unlocks,
            total_active_achievements,
        },
    }))
}

#[utoipa::path(get, path = "/api/v1/coffee-twins", responses((status = 200, body = CoffeeTwinsResponse)))]
pub(crate) async fn coffee_twins(
    State(state): State<AppState>,
    user: CurrentUser,
) -> ApiResult<Json<CoffeeTwinsResponse>> {
    if user.user_type != "member" || !user.leaderboard_opt_in {
        return Ok(Json(CoffeeTwinsResponse { twins: Vec::new() }));
    }

    let rows = sqlx::query_as::<_, TwinRow>(
        "SELECT u.id AS user_id, u.display_name, c.consumed_at
         FROM users u
         JOIN consumptions c ON c.user_id = u.id AND c.beverage_id = 'coffee'
         WHERE u.user_type = 'member' AND u.leaderboard_opt_in = 1",
    )
    .fetch_all(&state.db)
    .await?;

    let mut names = HashMap::<String, String>::new();
    let mut vectors = HashMap::<String, [f64; 24]>::new();
    for row in rows {
        names.insert(row.user_id.clone(), row.display_name);
        let Some(dt) = Utc.timestamp_opt(row.consumed_at, 0).single() else {
            return Err(ApiError::BadRequest("invalid timestamp in database".into()));
        };
        vectors.entry(row.user_id).or_insert([0.0; 24])[dt.hour() as usize] += 1.0;
    }

    let Some(my_vector) = vectors.get(&user.id).copied() else {
        return Ok(Json(CoffeeTwinsResponse { twins: Vec::new() }));
    };

    let mut twins = vectors
        .into_iter()
        .filter(|(user_id, _)| user_id != &user.id)
        .map(|(user_id, vector)| CoffeeTwin {
            display_name: names.remove(&user_id).unwrap_or_else(|| "Unknown".into()),
            similarity: cosine_similarity(&my_vector, &vector),
            user_id,
        })
        .collect::<Vec<_>>();
    twins.sort_by(|a, b| {
        b.similarity
            .partial_cmp(&a.similarity)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.display_name.cmp(&b.display_name))
    });
    twins.truncate(10);

    Ok(Json(CoffeeTwinsResponse { twins }))
}

pub fn cosine_similarity(a: &[f64; 24], b: &[f64; 24]) -> f64 {
    let dot = a
        .iter()
        .zip(b.iter())
        .map(|(left, right)| left * right)
        .sum::<f64>();
    let norm_a = a.iter().map(|value| value * value).sum::<f64>().sqrt();
    let norm_b = b.iter().map(|value| value * value).sum::<f64>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    ((dot / (norm_a * norm_b)) * 1000.0).round() / 1000.0
}

#[derive(sqlx::FromRow)]
struct LeaderboardRow {
    user_id: String,
    display_name: String,
    coffee_count: i64,
}

#[derive(sqlx::FromRow)]
struct VisibleUserRow {
    user_id: String,
    display_name: String,
}

#[derive(sqlx::FromRow)]
struct UserConsumptionRow {
    user_id: String,
    consumed_at: i64,
}

#[derive(sqlx::FromRow)]
struct AchievementLeaderboardRow {
    user_id: String,
    display_name: String,
    unlocked_count: i64,
    latest_unlocked_at: Option<i64>,
}

#[derive(sqlx::FromRow)]
struct AchievementUnlockSummaryRow {
    user_id: String,
    achievement_id: String,
    name: String,
    unlocked_at: i64,
}

#[derive(sqlx::FromRow)]
struct RareAchievementRow {
    achievement_id: String,
    name: String,
    unlock_count: i64,
}

#[derive(sqlx::FromRow)]
struct RecentAchievementUnlockRow {
    user_id: String,
    display_name: String,
    achievement_id: String,
    achievement_name: String,
    unlocked_at: i64,
}

#[derive(sqlx::FromRow)]
struct TwinRow {
    user_id: String,
    display_name: String,
    consumed_at: i64,
}

fn streak_entries(
    users: Vec<VisibleUserRow>,
    rows: Vec<UserConsumptionRow>,
    today: NaiveDate,
    tz: &Tz,
) -> Vec<StreakLeaderboardEntry> {
    let mut consumptions = HashMap::<String, Vec<i64>>::new();
    for row in rows {
        consumptions
            .entry(row.user_id)
            .or_default()
            .push(row.consumed_at);
    }

    let mut entries = users
        .into_iter()
        .filter_map(|user| {
            let timestamps = consumptions.remove(&user.user_id).unwrap_or_default();
            if timestamps.is_empty() {
                return None;
            }
            let streaks = streak_lengths_from_timestamps(&timestamps, today, tz);
            Some(StreakLeaderboardEntry {
                rank: 0,
                user_id: user.user_id,
                display_name: user.display_name,
                current_streak_days: streaks.current,
                longest_streak_days: streaks.longest,
                drink_count: timestamps.len() as i64,
            })
        })
        .collect::<Vec<_>>();
    entries.sort_by(|a, b| {
        b.current_streak_days
            .cmp(&a.current_streak_days)
            .then_with(|| b.longest_streak_days.cmp(&a.longest_streak_days))
            .then_with(|| a.display_name.cmp(&b.display_name))
    });
    for (idx, entry) in entries.iter_mut().enumerate() {
        entry.rank = idx as i64 + 1;
    }
    entries
}

struct StreakLengths {
    current: i64,
    longest: i64,
}

fn streak_lengths_from_timestamps(timestamps: &[i64], today: NaiveDate, tz: &Tz) -> StreakLengths {
    let dates = timestamps
        .iter()
        .filter_map(|timestamp| Utc.timestamp_opt(*timestamp, 0).single())
        .map(|dt| dt.with_timezone(tz).date_naive())
        .filter(|date| is_lab_day(*date))
        .collect::<HashSet<_>>();
    if dates.is_empty() {
        return StreakLengths {
            current: 0,
            longest: 0,
        };
    }

    let mut sorted = dates.iter().copied().collect::<Vec<_>>();
    sorted.sort_unstable();

    let mut longest = 1;
    let mut run = 1;
    for pair in sorted.windows(2) {
        if pair[1] == next_lab_day(pair[0]) {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 1;
        }
    }

    let mut current = 0;
    let mut probe = today;
    if !dates.contains(&probe) {
        probe = previous_lab_day(today);
    }
    while dates.contains(&probe) {
        current += 1;
        probe = previous_lab_day(probe);
    }

    StreakLengths { current, longest }
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

fn previous_lab_day(mut date: NaiveDate) -> NaiveDate {
    loop {
        date -= Duration::days(1);
        if is_lab_day(date) {
            return date;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::State;
    use chrono::NaiveDate;
    use uuid::Uuid;

    use crate::{config::Config, db::connect};

    async fn state() -> AppState {
        let db_path = format!("/tmp/coffee-social-{}.db", Uuid::new_v4());
        let db = connect(&format!("sqlite://{db_path}")).await.unwrap();
        sqlx::query(
            "INSERT INTO users
             (id, user_type, gitlab_id, display_name, leaderboard_opt_in,
              achievement_sharing_opt_out, mattermost_opt_in, stock_manager, created_at, updated_at)
             VALUES
             ('ada', 'member', 1, 'Ada', 1, 0, 0, 0, 1, 1),
             ('bob', 'member', 2, 'Bob', 1, 0, 0, 0, 1, 1),
             ('private', 'member', 3, 'Private', 1, 1, 0, 0, 1, 1),
             ('hidden', 'member', 4, 'Hidden', 0, 0, 0, 0, 1, 1),
             ('guest', 'guest', NULL, 'Guest', 0, 0, 0, 0, 1, 1)",
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
             VALUES ('coffee', 'Coffee', 50, 90, 1), ('eis', 'Eis', 50, 0, 1)",
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
                timezone: "Europe/Berlin".into(),
                session_ttl: std::time::Duration::from_secs(60),
            },
            db,
            http: reqwest::Client::new(),
        }
    }

    async fn insert_consumptions(state: &AppState, user_id: &str, count: usize, start_at: i64) {
        for index in 0..count {
            let timestamp = start_at + index as i64;
            sqlx::query(
                "INSERT INTO consumptions
                 (id, user_id, machine_id, beverage_id, consumed_at, created_at)
                 VALUES (?, ?, 'main', 'coffee', ?, ?)",
            )
            .bind(format!("{user_id}-{index}"))
            .bind(user_id)
            .bind(timestamp)
            .bind(timestamp)
            .execute(&state.db)
            .await
            .unwrap();
        }
    }

    async fn insert_achievement(state: &AppState, id: &str, name: &str) {
        sqlx::query(
            "INSERT INTO achievement_definitions
             (id, name, description, hint, criteria_kind, threshold, active, created_at)
             VALUES (?, ?, 'Description', 'Hint', 'lifetime_cups', 1, 1, 1)",
        )
        .bind(id)
        .bind(name)
        .execute(&state.db)
        .await
        .unwrap();
    }

    async fn insert_unlock(
        state: &AppState,
        id: &str,
        user_id: &str,
        achievement_id: &str,
        at: i64,
    ) {
        sqlx::query(
            "INSERT INTO achievement_unlocks
             (id, user_id, achievement_id, unlocked_at)
             VALUES (?, ?, ?, ?)",
        )
        .bind(id)
        .bind(user_id)
        .bind(achievement_id)
        .bind(at)
        .execute(&state.db)
        .await
        .unwrap();
    }

    #[test]
    fn cosine_similarity_detects_matching_patterns() {
        let mut a = [0.0; 24];
        let mut b = [0.0; 24];
        a[8] = 2.0;
        b[8] = 4.0;
        assert_eq!(cosine_similarity(&a, &b), 1.0);
        b[8] = 0.0;
        b[20] = 4.0;
        assert_eq!(cosine_similarity(&a, &b), 0.0);
    }

    #[test]
    fn streak_ranking_uses_timezone_and_lab_day_rules() {
        let tz = chrono_tz::Europe::Berlin;
        let friday = Utc
            .with_ymd_and_hms(2026, 5, 1, 10, 0, 0)
            .unwrap()
            .timestamp();
        let monday = Utc
            .with_ymd_and_hms(2026, 5, 4, 10, 0, 0)
            .unwrap()
            .timestamp();
        let sunday_utc_monday_local = Utc
            .with_ymd_and_hms(2026, 5, 3, 22, 30, 0)
            .unwrap()
            .timestamp();

        let entries = streak_entries(
            vec![
                VisibleUserRow {
                    user_id: "ada".into(),
                    display_name: "Ada".into(),
                },
                VisibleUserRow {
                    user_id: "zoe".into(),
                    display_name: "Zoe".into(),
                },
            ],
            vec![
                UserConsumptionRow {
                    user_id: "ada".into(),
                    consumed_at: friday,
                },
                UserConsumptionRow {
                    user_id: "ada".into(),
                    consumed_at: monday,
                },
                UserConsumptionRow {
                    user_id: "zoe".into(),
                    consumed_at: sunday_utc_monday_local,
                },
            ],
            NaiveDate::from_ymd_opt(2026, 5, 4).unwrap(),
            &tz,
        );

        assert_eq!(entries[0].display_name, "Ada");
        assert_eq!(entries[0].current_streak_days, 2);
        assert_eq!(entries[0].longest_streak_days, 2);
        assert_eq!(entries[1].display_name, "Zoe");
        assert_eq!(entries[1].current_streak_days, 1);
    }

    #[tokio::test]
    async fn expanded_leaderboard_filters_visibility_and_preserves_coffee_order() {
        let state = state().await;
        insert_consumptions(&state, "ada", 2, 10).await;
        insert_consumptions(&state, "bob", 1, 20).await;
        insert_consumptions(&state, "private", 3, 30).await;
        insert_consumptions(&state, "hidden", 5, 40).await;
        insert_consumptions(&state, "guest", 4, 50).await;

        let Json(response) = leaderboard(State(state)).await.unwrap();

        let names = response
            .entries
            .iter()
            .map(|entry| entry.display_name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["Private", "Ada", "Bob"]);
        assert_eq!(response.summary.visible_participants, 3);
        assert!(
            response
                .streak_entries
                .iter()
                .all(|entry| entry.display_name != "Hidden" && entry.display_name != "Guest")
        );
    }

    #[tokio::test]
    async fn achievement_leaderboard_respects_achievement_sharing_opt_out() {
        let state = state().await;
        insert_consumptions(&state, "ada", 1, 10).await;
        insert_consumptions(&state, "bob", 1, 20).await;
        insert_consumptions(&state, "private", 1, 30).await;
        insert_achievement(&state, "first", "First").await;
        insert_achievement(&state, "rare", "Rare").await;
        insert_unlock(&state, "ada-first", "ada", "first", 10).await;
        insert_unlock(&state, "ada-rare", "ada", "rare", 20).await;
        insert_unlock(&state, "bob-first", "bob", "first", 30).await;
        insert_unlock(&state, "private-first", "private", "first", 40).await;

        let Json(response) = leaderboard(State(state)).await.unwrap();

        let names = response
            .achievement_entries
            .iter()
            .map(|entry| entry.display_name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["Ada", "Bob"]);
        assert_eq!(response.achievement_entries[0].unlocks.len(), 2);
        assert_eq!(response.achievement_entries[0].unlocks[0].name, "Rare");
        assert_eq!(response.achievement_entries[0].unlocks[1].name, "First");
        assert_eq!(response.achievement_entries[1].unlocks.len(), 1);
        assert_eq!(response.achievement_entries[1].unlocks[0].name, "First");
        assert_eq!(response.summary.total_public_unlocks, 3);
        assert_eq!(response.rare_achievements[0].name, "Rare");
        assert_eq!(response.rare_achievements[0].unlock_count, 1);
        assert_eq!(response.recent_unlocks[0].display_name, "Bob");
        assert!(
            response
                .recent_unlocks
                .iter()
                .all(|unlock| unlock.display_name != "Private")
        );
    }
}
