use std::collections::{BTreeMap, HashSet};

use axum::{
    Json, Router,
    extract::{Query, State},
    routing::get,
};
use chrono::{Datelike, Duration, NaiveDate, TimeZone, Timelike, Utc, Weekday};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{
    AppState,
    auth::CurrentUser,
    db::now_ts,
    error::{ApiError, ApiResult},
    expenses::chair_expense_balance_cents,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/stats/me/summary", get(summary))
        .route("/stats/me/buckets", get(buckets))
        .route("/stats/me/hourly", get(hourly))
        .route("/stats/me/beverages", get(my_beverages))
        .route("/stats/me/streaks", get(streaks))
        .route("/stats/me/caffeine", get(caffeine))
        .route("/stats/global/summary", get(global_summary))
        .route("/stats/global/hourly", get(global_hourly))
        .route("/stats/global/buckets", get(global_buckets))
        .route("/stats/global/beverages", get(global_beverages))
}

#[derive(Serialize, ToSchema)]
pub struct SummaryResponse {
    pub total_drinks: i64,
    pub coffee_count: i64,
    pub other_count: i64,
    pub beverages: Vec<BeverageSummary>,
    pub recent_consumptions: Vec<RecentConsumption>,
    pub total_price_cents: i64,
    pub current_streak_days: i64,
    pub longest_streak_days: i64,
    pub caffeine_mg: f64,
}

#[derive(Serialize, ToSchema)]
pub struct BeverageSummary {
    pub beverage_id: String,
    pub name: String,
    pub count: i64,
    pub price_cents: i64,
}

#[derive(Serialize, ToSchema)]
pub struct RecentConsumption {
    pub beverage_id: String,
    pub consumed_at: i64,
}

#[derive(Serialize, ToSchema)]
pub struct Bucket {
    pub date: String,
    pub count: i64,
    pub price_cents: i64,
}

#[derive(Serialize, ToSchema)]
pub struct BucketsResponse {
    pub buckets: Vec<Bucket>,
}

#[derive(Serialize, ToSchema)]
pub struct HourBucket {
    pub hour: i64,
    pub count: i64,
}

#[derive(Serialize, ToSchema)]
pub struct HourlyResponse {
    pub buckets: Vec<HourBucket>,
}

#[derive(Serialize, ToSchema)]
pub struct StreaksResponse {
    pub current_days: i64,
    pub longest_days: i64,
}

#[derive(Serialize, ToSchema)]
pub struct CaffeineResponse {
    pub caffeine_mg: f64,
    pub half_life_hours: f64,
    pub calculated_at: i64,
}

#[derive(Serialize, ToSchema)]
pub struct GlobalTopDrinker {
    pub user_id: String,
    pub display_name: String,
    pub coffee_count: i64,
}

#[derive(Serialize, ToSchema)]
pub struct GlobalStatsResponse {
    pub total_coffees: i64,
    pub total_drinks: i64,
    pub active_users_30d: i64,
    pub today_drinks: i64,
    pub last_7_days_drinks: i64,
    pub unpaid_total_cents: i64,
    pub paid_total_cents: i64,
    pub expense_balance_cents: i64,
    pub chair_balance_cents: i64,
    pub kn_drinks: i64,
    pub kn_coffees: i64,
    pub guest_drinks: i64,
    pub guest_coffees: i64,
    pub top_drinker: Option<GlobalTopDrinker>,
}

#[derive(Deserialize, IntoParams)]
pub struct RangeQuery {
    /// Days back to include, or "all" for the full history. Defaults to 30.
    #[param(example = "30")]
    pub range: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct GlobalHourBucket {
    pub hour: i64,
    pub kn_count: i64,
    pub guest_count: i64,
}

#[derive(Serialize, ToSchema)]
pub struct GlobalHourlyResponse {
    pub buckets: Vec<GlobalHourBucket>,
}

#[derive(Serialize, ToSchema)]
pub struct GlobalBucket {
    pub date: String,
    pub kn_count: i64,
    pub guest_count: i64,
}

#[derive(Serialize, ToSchema)]
pub struct GlobalBucketsResponse {
    pub buckets: Vec<GlobalBucket>,
}

#[derive(Serialize, ToSchema)]
pub struct MyBeverageResponse {
    pub beverages: Vec<BeverageSummary>,
}

#[derive(Serialize, ToSchema)]
pub struct GlobalBeverageResponse {
    pub beverages: Vec<BeverageSummary>,
}

#[derive(sqlx::FromRow)]
struct ConsumptionForStats {
    consumed_at: i64,
    beverage_id: String,
    beverage_name: String,
    price_cents: i64,
    caffeine_mg: i64,
}

#[utoipa::path(get, path = "/api/v1/stats/me/summary", responses((status = 200, body = SummaryResponse)))]
pub(crate) async fn summary(
    State(state): State<AppState>,
    user: CurrentUser,
) -> ApiResult<Json<SummaryResponse>> {
    let tz = resolve_tz(&state.config.timezone);
    let rows = user_consumptions(&state, &user.id).await?;
    let streaks = streak_lengths(&rows, &tz);
    let total_drinks = rows.len() as i64;
    let coffee_count = rows
        .iter()
        .filter(|row| row.beverage_id == "coffee")
        .count() as i64;
    let other_count = rows.iter().filter(|row| row.beverage_id == "other").count() as i64;
    let beverages = beverage_summaries(&rows);
    let today_start = local_today_start(&tz, now_ts());
    let recent_consumptions = rows
        .iter()
        .filter(|row| row.consumed_at >= today_start)
        .map(|row| RecentConsumption {
            beverage_id: row.beverage_id.clone(),
            consumed_at: row.consumed_at,
        })
        .collect();
    let total_price_cents = rows.iter().map(|row| row.price_cents).sum();
    Ok(Json(SummaryResponse {
        total_drinks,
        coffee_count,
        other_count,
        beverages,
        recent_consumptions,
        total_price_cents,
        current_streak_days: streaks.current,
        longest_streak_days: streaks.longest,
        caffeine_mg: caffeine_level(&rows, now_ts()),
    }))
}

#[utoipa::path(get, path = "/api/v1/stats/me/buckets", responses((status = 200, body = BucketsResponse)))]
pub(crate) async fn buckets(
    State(state): State<AppState>,
    user: CurrentUser,
) -> ApiResult<Json<BucketsResponse>> {
    let tz = resolve_tz(&state.config.timezone);
    let rows = user_consumptions(&state, &user.id).await?;
    let today = local_date(now_ts(), &tz)
        .ok_or_else(|| ApiError::BadRequest("invalid current timestamp".into()))?;
    Ok(Json(BucketsResponse {
        buckets: daily_buckets(&rows, &tz, today)?,
    }))
}

fn daily_buckets(
    rows: &[ConsumptionForStats],
    tz: &Tz,
    today: NaiveDate,
) -> ApiResult<Vec<Bucket>> {
    let mut by_date: BTreeMap<NaiveDate, (i64, i64)> = BTreeMap::new();
    for row in rows {
        let date = local_date(row.consumed_at, &tz)
            .ok_or_else(|| ApiError::BadRequest("invalid timestamp in database".into()))?;
        if date > today {
            continue;
        }
        let entry = by_date.entry(date).or_insert((0, 0));
        entry.0 += 1;
        entry.1 += row.price_cents;
    }
    let Some(first_date) = by_date.keys().next().copied() else {
        return Ok(Vec::new());
    };
    let mut buckets = Vec::new();
    let mut date = first_date;
    while date <= today {
        let (count, price_cents) = by_date.get(&date).copied().unwrap_or((0, 0));
        buckets.push(Bucket {
            date: date.to_string(),
            count,
            price_cents,
        });
        date += Duration::days(1);
    }
    Ok(buckets)
}

#[utoipa::path(
    get,
    path = "/api/v1/stats/me/hourly",
    params(RangeQuery),
    responses((status = 200, body = HourlyResponse))
)]
pub(crate) async fn hourly(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(query): Query<RangeQuery>,
) -> ApiResult<Json<HourlyResponse>> {
    let tz = resolve_tz(&state.config.timezone);
    let rows = user_consumptions(&state, &user.id).await?;
    let rows = filter_by_range(rows, query.range.as_deref());
    Ok(Json(HourlyResponse {
        buckets: hourly_counts(&rows, &tz)
            .into_iter()
            .enumerate()
            .map(|(hour, count)| HourBucket {
                hour: hour as i64,
                count,
            })
            .collect(),
    }))
}

/// Unlike the global endpoints, an absent range means the full history: the
/// personal endpoints predate the dashboard toggle and callers rely on all-time
/// numbers when they ask for no range at all.
fn filter_by_range(
    rows: Vec<ConsumptionForStats>,
    range: Option<&str>,
) -> Vec<ConsumptionForStats> {
    let cutoff = range_cutoff(now_ts(), range.and_then(|r| parse_range_days(Some(r))));
    rows.into_iter()
        .filter(|row| row.consumed_at >= cutoff)
        .collect()
}

#[utoipa::path(
    get,
    path = "/api/v1/stats/me/beverages",
    params(RangeQuery),
    responses((status = 200, body = MyBeverageResponse))
)]
pub(crate) async fn my_beverages(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(query): Query<RangeQuery>,
) -> ApiResult<Json<MyBeverageResponse>> {
    let rows = user_consumptions(&state, &user.id).await?;
    let rows = filter_by_range(rows, query.range.as_deref());
    Ok(Json(MyBeverageResponse {
        beverages: beverage_summaries(&rows),
    }))
}

#[utoipa::path(get, path = "/api/v1/stats/me/streaks", responses((status = 200, body = StreaksResponse)))]
pub(crate) async fn streaks(
    State(state): State<AppState>,
    user: CurrentUser,
) -> ApiResult<Json<StreaksResponse>> {
    let tz = resolve_tz(&state.config.timezone);
    let rows = user_consumptions(&state, &user.id).await?;
    let streaks = streak_lengths(&rows, &tz);
    Ok(Json(StreaksResponse {
        current_days: streaks.current,
        longest_days: streaks.longest,
    }))
}

#[utoipa::path(get, path = "/api/v1/stats/me/caffeine", responses((status = 200, body = CaffeineResponse)))]
pub(crate) async fn caffeine(
    State(state): State<AppState>,
    user: CurrentUser,
) -> ApiResult<Json<CaffeineResponse>> {
    let now = now_ts();
    let rows = user_consumptions(&state, &user.id).await?;
    Ok(Json(CaffeineResponse {
        caffeine_mg: caffeine_level(&rows, now),
        half_life_hours: 5.0,
        calculated_at: now,
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/stats/global/summary",
    responses((status = 200, body = GlobalStatsResponse))
)]
pub(crate) async fn global_summary(
    State(state): State<AppState>,
    _user: CurrentUser,
) -> ApiResult<Json<GlobalStatsResponse>> {
    let tz = resolve_tz(&state.config.timezone);
    let now = now_ts();
    let today_start = local_today_start(&tz, now);
    let last_7_days_start = now - 7 * 24 * 60 * 60;
    let active_start = now - 30 * 24 * 60 * 60;

    let (total_drinks,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM consumptions")
        .fetch_one(&state.db)
        .await?;
    let (total_coffees,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM consumptions WHERE beverage_id = 'coffee'")
            .fetch_one(&state.db)
            .await?;
    let (active_users_30d,): (i64,) =
        sqlx::query_as("SELECT COUNT(DISTINCT user_id) FROM consumptions WHERE consumed_at >= ?")
            .bind(active_start)
            .fetch_one(&state.db)
            .await?;
    let (today_drinks,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM consumptions WHERE consumed_at >= ?")
            .bind(today_start)
            .fetch_one(&state.db)
            .await?;
    let (last_7_days_drinks,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM consumptions WHERE consumed_at >= ?")
            .bind(last_7_days_start)
            .fetch_one(&state.db)
            .await?;
    let (unpaid_total_cents,): (Option<i64>,) = sqlx::query_as(
        "SELECT SUM(b.price_cents)
         FROM consumptions c
         JOIN beverages b ON b.id = c.beverage_id
         WHERE c.consumed_at >= COALESCE(
           (SELECT MAX(p.period_end_ts) FROM payments p WHERE p.user_id = c.user_id),
           0
         )",
    )
    .fetch_one(&state.db)
    .await?;
    let (paid_total_cents,): (Option<i64>,) =
        sqlx::query_as("SELECT SUM(amount_cents) FROM payments")
            .fetch_one(&state.db)
            .await?;
    let (kn_drinks, kn_coffees, guest_drinks, guest_coffees): (i64, i64, i64, i64) =
        sqlx::query_as(
            "SELECT
               COALESCE(SUM(CASE WHEN u.user_type = 'member' THEN 1 ELSE 0 END), 0),
               COALESCE(SUM(CASE WHEN u.user_type = 'member' AND c.beverage_id = 'coffee' THEN 1 ELSE 0 END), 0),
               COALESCE(SUM(CASE WHEN u.user_type = 'guest' THEN 1 ELSE 0 END), 0),
               COALESCE(SUM(CASE WHEN u.user_type = 'guest' AND c.beverage_id = 'coffee' THEN 1 ELSE 0 END), 0)
             FROM consumptions c
             JOIN users u ON u.id = c.user_id",
        )
        .fetch_one(&state.db)
        .await?;
    let top_drinker = sqlx::query_as::<_, TopDrinkerRow>(
        "SELECT u.id AS user_id, u.display_name, COUNT(c.id) AS coffee_count
         FROM users u
         JOIN consumptions c ON c.user_id = u.id AND c.beverage_id = 'coffee'
         WHERE u.user_type = 'member' AND u.leaderboard_opt_in = 1
         GROUP BY u.id, u.display_name
         ORDER BY coffee_count DESC, u.display_name ASC
         LIMIT 1",
    )
    .fetch_optional(&state.db)
    .await?
    .map(|row| GlobalTopDrinker {
        user_id: row.user_id,
        display_name: row.display_name,
        coffee_count: row.coffee_count,
    });

    let unpaid_total_cents = unpaid_total_cents.unwrap_or(0);
    let paid_total_cents = paid_total_cents.unwrap_or(0);
    let expense_balance_cents = chair_expense_balance_cents(&state).await?;
    let chair_balance_cents = unpaid_total_cents + expense_balance_cents;

    Ok(Json(GlobalStatsResponse {
        total_coffees,
        total_drinks,
        active_users_30d,
        today_drinks,
        last_7_days_drinks,
        unpaid_total_cents,
        paid_total_cents,
        expense_balance_cents,
        chair_balance_cents,
        kn_drinks,
        kn_coffees,
        guest_drinks,
        guest_coffees,
        top_drinker,
    }))
}

fn parse_range_days(range: Option<&str>) -> Option<i64> {
    match range {
        None => Some(30),
        Some(value) => {
            let trimmed = value.trim();
            if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("all") {
                None
            } else {
                trimmed
                    .parse::<i64>()
                    .ok()
                    .filter(|days| *days > 0)
                    .or(Some(30))
            }
        }
    }
}

fn range_cutoff(now: i64, days: Option<i64>) -> i64 {
    days.map(|d| now - d * 24 * 60 * 60).unwrap_or(0)
}

#[derive(sqlx::FromRow)]
struct GlobalConsumption {
    consumed_at: i64,
    user_type: String,
}

async fn global_consumptions(state: &AppState, cutoff: i64) -> ApiResult<Vec<GlobalConsumption>> {
    sqlx::query_as::<_, GlobalConsumption>(
        "SELECT c.consumed_at, u.user_type
         FROM consumptions c
         JOIN users u ON u.id = c.user_id
         WHERE c.consumed_at >= ?
         ORDER BY c.consumed_at ASC",
    )
    .bind(cutoff)
    .fetch_all(&state.db)
    .await
    .map_err(ApiError::from)
}

async fn global_consumption_rows(
    state: &AppState,
    cutoff: i64,
) -> ApiResult<Vec<ConsumptionForStats>> {
    sqlx::query_as::<_, ConsumptionForStats>(
        "SELECT c.consumed_at, c.beverage_id, b.name AS beverage_name, b.price_cents, b.caffeine_mg
         FROM consumptions c
         JOIN beverages b ON b.id = c.beverage_id
         WHERE c.consumed_at >= ?
         ORDER BY c.consumed_at ASC",
    )
    .bind(cutoff)
    .fetch_all(&state.db)
    .await
    .map_err(ApiError::from)
}

#[utoipa::path(
    get,
    path = "/api/v1/stats/global/hourly",
    params(RangeQuery),
    responses((status = 200, body = GlobalHourlyResponse))
)]
pub(crate) async fn global_hourly(
    State(state): State<AppState>,
    _user: CurrentUser,
    Query(query): Query<RangeQuery>,
) -> ApiResult<Json<GlobalHourlyResponse>> {
    let tz = resolve_tz(&state.config.timezone);
    let days = parse_range_days(query.range.as_deref());
    let cutoff = range_cutoff(now_ts(), days);
    let rows = global_consumptions(&state, cutoff).await?;

    let mut kn = [0_i64; 24];
    let mut guest = [0_i64; 24];
    for row in &rows {
        if let Some(dt) = Utc.timestamp_opt(row.consumed_at, 0).single() {
            let hour = dt.with_timezone(&tz).hour() as usize;
            match row.user_type.as_str() {
                "guest" => guest[hour] += 1,
                _ => kn[hour] += 1,
            }
        }
    }

    Ok(Json(GlobalHourlyResponse {
        buckets: (0..24)
            .map(|hour| GlobalHourBucket {
                hour: hour as i64,
                kn_count: kn[hour],
                guest_count: guest[hour],
            })
            .collect(),
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/stats/global/buckets",
    params(RangeQuery),
    responses((status = 200, body = GlobalBucketsResponse))
)]
pub(crate) async fn global_buckets(
    State(state): State<AppState>,
    _user: CurrentUser,
    Query(query): Query<RangeQuery>,
) -> ApiResult<Json<GlobalBucketsResponse>> {
    let tz = resolve_tz(&state.config.timezone);
    let days = parse_range_days(query.range.as_deref());
    let cutoff = range_cutoff(now_ts(), days);
    let rows = global_consumptions(&state, cutoff).await?;

    let mut by_date: BTreeMap<String, (i64, i64)> = BTreeMap::new();
    for row in &rows {
        let date = local_date(row.consumed_at, &tz)
            .ok_or_else(|| ApiError::BadRequest("invalid timestamp in database".into()))?
            .to_string();
        let entry = by_date.entry(date).or_insert((0, 0));
        if row.user_type == "guest" {
            entry.1 += 1;
        } else {
            entry.0 += 1;
        }
    }
    Ok(Json(GlobalBucketsResponse {
        buckets: by_date
            .into_iter()
            .map(|(date, (kn_count, guest_count))| GlobalBucket {
                date,
                kn_count,
                guest_count,
            })
            .collect(),
    }))
}

#[utoipa::path(
        get,
        path = "/api/v1/stats/global/beverages",
        params(RangeQuery),
        responses((status = 200, body = GlobalBeverageResponse))
    )]
pub(crate) async fn global_beverages(
    State(state): State<AppState>,
    _user: CurrentUser,
    Query(query): Query<RangeQuery>,
) -> ApiResult<Json<GlobalBeverageResponse>> {
    let days = parse_range_days(query.range.as_deref());
    let cutoff = range_cutoff(now_ts(), days);
    let rows = global_consumption_rows(&state, cutoff).await?;

    Ok(Json(GlobalBeverageResponse {
        beverages: beverage_summaries(&rows),
    }))
}

async fn user_consumptions(state: &AppState, user_id: &str) -> ApiResult<Vec<ConsumptionForStats>> {
    sqlx::query_as::<_, ConsumptionForStats>(
        "SELECT c.consumed_at, c.beverage_id, b.name AS beverage_name, b.price_cents, b.caffeine_mg
         FROM consumptions c
         JOIN beverages b ON b.id = c.beverage_id
         WHERE c.user_id = ?
         ORDER BY c.consumed_at ASC",
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await
    .map_err(ApiError::from)
}

#[derive(sqlx::FromRow)]
struct TopDrinkerRow {
    user_id: String,
    display_name: String,
    coffee_count: i64,
}

pub fn resolve_tz(name: &str) -> Tz {
    name.parse::<Tz>().unwrap_or(chrono_tz::UTC)
}

fn local_date(timestamp: i64, tz: &Tz) -> Option<NaiveDate> {
    Utc.timestamp_opt(timestamp, 0)
        .single()
        .map(|dt| dt.with_timezone(tz).date_naive())
}

fn local_today_start(tz: &Tz, now: i64) -> i64 {
    let now_dt = Utc
        .timestamp_opt(now, 0)
        .single()
        .map(|dt| dt.with_timezone(tz))
        .unwrap_or_else(|| Utc::now().with_timezone(tz));
    now_dt
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .and_then(|naive| tz.from_local_datetime(&naive).single())
        .map(|dt| dt.timestamp())
        .unwrap_or(now)
}

pub fn caffeine_decay(initial_mg: f64, age_seconds: i64) -> f64 {
    if age_seconds <= 0 {
        return initial_mg;
    }
    let age_hours = age_seconds as f64 / 3600.0;
    initial_mg * 0.5_f64.powf(age_hours / 5.0)
}

fn caffeine_level(rows: &[ConsumptionForStats], now: i64) -> f64 {
    rows.iter()
        .filter(|row| row.caffeine_mg > 0 && row.consumed_at <= now)
        .map(|row| caffeine_decay(row.caffeine_mg as f64, now - row.consumed_at))
        .sum::<f64>()
        .round()
}

fn beverage_summaries(rows: &[ConsumptionForStats]) -> Vec<BeverageSummary> {
    let mut summaries = BTreeMap::<String, BeverageSummary>::new();
    for row in rows {
        let entry = summaries
            .entry(row.beverage_id.clone())
            .or_insert_with(|| BeverageSummary {
                beverage_id: row.beverage_id.clone(),
                name: row.beverage_name.clone(),
                count: 0,
                price_cents: 0,
            });
        entry.count += 1;
        entry.price_cents += row.price_cents;
    }
    summaries.into_values().collect()
}

pub fn hourly_counts_from_timestamps(timestamps: &[i64], tz: &Tz) -> [i64; 24] {
    let mut buckets = [0_i64; 24];
    for timestamp in timestamps {
        if let Some(dt) = Utc.timestamp_opt(*timestamp, 0).single() {
            buckets[dt.with_timezone(tz).hour() as usize] += 1;
        }
    }
    buckets
}

fn hourly_counts(rows: &[ConsumptionForStats], tz: &Tz) -> [i64; 24] {
    let timestamps = rows.iter().map(|row| row.consumed_at).collect::<Vec<_>>();
    hourly_counts_from_timestamps(&timestamps, tz)
}

pub struct StreakLengths {
    pub current: i64,
    pub longest: i64,
}

fn streak_lengths(rows: &[ConsumptionForStats], tz: &Tz) -> StreakLengths {
    streak_lengths_at(rows, Utc::now().with_timezone(tz).date_naive(), tz)
}

fn streak_lengths_at(rows: &[ConsumptionForStats], today: NaiveDate, tz: &Tz) -> StreakLengths {
    let dates = rows
        .iter()
        .filter_map(|row| Utc.timestamp_opt(row.consumed_at, 0).single())
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
    use crate::{config::Config, db::connect};

    #[test]
    fn caffeine_half_life_is_five_hours() {
        assert_eq!(caffeine_decay(100.0, 5 * 3600).round(), 50.0);
        assert_eq!(caffeine_decay(100.0, 10 * 3600).round(), 25.0);
    }

    #[test]
    fn hourly_histogram_counts_utc_hours() {
        let ts = [
            Utc.with_ymd_and_hms(2026, 4, 28, 7, 0, 0)
                .unwrap()
                .timestamp(),
            Utc.with_ymd_and_hms(2026, 4, 28, 7, 30, 0)
                .unwrap()
                .timestamp(),
        ];
        let buckets = hourly_counts_from_timestamps(&ts, &chrono_tz::UTC);
        assert_eq!(buckets[7], 2);
    }

    #[test]
    fn hourly_histogram_uses_configured_timezone() {
        // 07:30 UTC == 09:30 in Europe/Berlin (CEST in late April).
        let ts = [Utc
            .with_ymd_and_hms(2026, 4, 28, 7, 30, 0)
            .unwrap()
            .timestamp()];
        let buckets = hourly_counts_from_timestamps(&ts, &chrono_tz::Europe::Berlin);
        assert_eq!(buckets[9], 1);
        assert_eq!(buckets[7], 0);
    }

    #[test]
    fn daily_buckets_include_zero_count_past_days_only() {
        let rows = [
            stat_row(2026, 5, 7),  // Thursday
            stat_row(2026, 5, 11), // Monday
            stat_row(2026, 5, 12), // Tuesday
            stat_row(2026, 5, 13), // Future relative to the supplied today.
        ];

        let buckets = daily_buckets(
            &rows,
            &chrono_tz::Europe::Berlin,
            NaiveDate::from_ymd_opt(2026, 5, 12).unwrap(),
        )
        .unwrap();

        assert_eq!(
            buckets
                .iter()
                .map(|bucket| (bucket.date.as_str(), bucket.count))
                .collect::<Vec<_>>(),
            vec![
                ("2026-05-07", 1),
                ("2026-05-08", 0),
                ("2026-05-09", 0),
                ("2026-05-10", 0),
                ("2026-05-11", 1),
                ("2026-05-12", 1),
            ],
        );
    }

    fn stat_row(year: i32, month: u32, day: u32) -> ConsumptionForStats {
        ConsumptionForStats {
            consumed_at: Utc
                .with_ymd_and_hms(year, month, day, 12, 0, 0)
                .unwrap()
                .timestamp(),
            beverage_id: "coffee".into(),
            beverage_name: "Coffee".into(),
            price_cents: 50,
            caffeine_mg: 90,
        }
    }

    #[test]
    fn streaks_skip_weekends_for_longest_and_current_counts() {
        let rows = [
            stat_row(2026, 5, 1), // Friday
            stat_row(2026, 5, 4), // Monday
            stat_row(2026, 5, 5), // Tuesday
        ];

        let streaks = streak_lengths_at(
            &rows,
            NaiveDate::from_ymd_opt(2026, 5, 5).unwrap(),
            &chrono_tz::UTC,
        );

        assert_eq!(streaks.longest, 3);
        assert_eq!(streaks.current, 3);
    }

    #[test]
    fn current_streak_does_not_end_during_weekend_or_monday_morning() {
        let rows = [stat_row(2026, 5, 1)]; // Friday

        let saturday = streak_lengths_at(
            &rows,
            NaiveDate::from_ymd_opt(2026, 5, 2).unwrap(),
            &chrono_tz::UTC,
        );
        let monday = streak_lengths_at(
            &rows,
            NaiveDate::from_ymd_opt(2026, 5, 4).unwrap(),
            &chrono_tz::UTC,
        );
        let tuesday = streak_lengths_at(
            &rows,
            NaiveDate::from_ymd_opt(2026, 5, 5).unwrap(),
            &chrono_tz::UTC,
        );

        assert_eq!(saturday.current, 1);
        assert_eq!(monday.current, 1);
        assert_eq!(tuesday.current, 0);
    }

    fn user(id: &str) -> CurrentUser {
        CurrentUser {
            id: id.to_string(),
            user_type: "member".into(),
            gitlab_id: Some(1),
            display_name: id.to_string(),
            leaderboard_opt_in: true,
            achievement_sharing_opt_out: false,
            mattermost_opt_in: false,
            stock_manager: false,
            paypal_link: None,
        }
    }

    async fn state() -> AppState {
        let db_path = format!("/tmp/coffee-stats-{}.db", uuid::Uuid::new_v4());
        let db = connect(&format!("sqlite://{db_path}")).await.unwrap();
        sqlx::query(
            "INSERT INTO users
             (id, user_type, gitlab_id, display_name, leaderboard_opt_in,
              achievement_sharing_opt_out, mattermost_opt_in, created_at, updated_at)
             VALUES
             ('ada', 'member', 1, 'Ada', 1, 0, 0, 1, 1),
             ('guest', 'guest', NULL, 'Guest', 0, 0, 0, 1, 1)",
        )
        .execute(&db)
        .await
        .unwrap();
        sqlx::query("INSERT INTO machines (id, name, created_at) VALUES ('main', 'Main', 1)")
            .execute(&db)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO beverages (id, name, price_cents, caffeine_mg, created_at)
             VALUES
             ('coffee', 'Coffee', 50, 90, 1),
             ('other', 'Other', 100, 0, 1)",
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

    async fn insert_consumption(
        state: &AppState,
        id: &str,
        user_id: &str,
        beverage_id: &str,
        consumed_at: i64,
    ) {
        sqlx::query(
            "INSERT INTO consumptions
             (id, user_id, machine_id, beverage_id, consumed_at, created_at)
             VALUES (?, ?, 'main', ?, ?, ?)",
        )
        .bind(id)
        .bind(user_id)
        .bind(beverage_id)
        .bind(consumed_at)
        .bind(consumed_at)
        .execute(&state.db)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn global_summary_aggregates_kn_stats() {
        let state = state().await;
        let now = now_ts();
        insert_consumption(&state, "ada-coffee", "ada", "coffee", now - 60).await;
        insert_consumption(&state, "ada-other", "ada", "other", now - 120).await;
        insert_consumption(&state, "guest-coffee", "guest", "coffee", now - 60).await;

        let Json(summary) = global_summary(State(state.clone()), user("ada"))
            .await
            .unwrap();
        assert_eq!(summary.total_drinks, 3);
        assert_eq!(summary.total_coffees, 2);
        assert_eq!(summary.active_users_30d, 2);
        assert_eq!(summary.last_7_days_drinks, 3);
        assert_eq!(summary.unpaid_total_cents, 200);
        assert_eq!(summary.paid_total_cents, 0);
        assert_eq!(summary.expense_balance_cents, 0);
        assert_eq!(summary.chair_balance_cents, 200);
        assert_eq!(summary.kn_drinks, 2);
        assert_eq!(summary.kn_coffees, 1);
        assert_eq!(summary.guest_drinks, 1);
        assert_eq!(summary.guest_coffees, 1);
        assert_eq!(summary.top_drinker.unwrap().display_name, "Ada");
    }

    #[tokio::test]
    async fn global_summary_includes_expenses_in_chair_balance() {
        let state = state().await;
        let now = now_ts();
        sqlx::query("UPDATE users SET stock_manager = 1 WHERE id = 'ada'")
            .execute(&state.db)
            .await
            .unwrap();
        insert_consumption(&state, "ada-coffee", "ada", "coffee", now - 60).await;
        sqlx::query(
            "INSERT INTO expense_entries
             (id, user_id, created_by_user_id, description, amount_cents, logged_at, created_at)
             VALUES ('expense', 'ada', 'ada', 'Coffee beans', 1500, ?, ?)",
        )
        .bind(now - 30)
        .bind(now - 30)
        .execute(&state.db)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO payments
             (id, user_id, received_by_user_id, amount_cents, drink_count, period_end_ts,
              paid_at, created_at)
             VALUES ('payment', 'guest', 'ada', 500, 10, ?, ?, ?)",
        )
        .bind(now - 10)
        .bind(now - 10)
        .bind(now - 10)
        .execute(&state.db)
        .await
        .unwrap();

        let Json(summary) = global_summary(State(state), user("ada")).await.unwrap();

        assert_eq!(summary.unpaid_total_cents, 50);
        assert_eq!(summary.paid_total_cents, 500);
        assert_eq!(summary.expense_balance_cents, -1000);
        assert_eq!(summary.chair_balance_cents, -950);
    }

    #[tokio::test]
    async fn global_hourly_splits_kn_and_guest() {
        let state = state().await;
        let now = now_ts();
        insert_consumption(&state, "ada-1", "ada", "coffee", now - 60).await;
        insert_consumption(&state, "ada-2", "ada", "coffee", now - 120).await;
        insert_consumption(&state, "guest-1", "guest", "coffee", now - 60).await;

        let Json(response) = global_hourly(
            State(state.clone()),
            user("ada"),
            Query(RangeQuery { range: None }),
        )
        .await
        .unwrap();
        let total_kn: i64 = response.buckets.iter().map(|b| b.kn_count).sum();
        let total_guest: i64 = response.buckets.iter().map(|b| b.guest_count).sum();
        assert_eq!(total_kn, 2);
        assert_eq!(total_guest, 1);
    }

    #[tokio::test]
    async fn global_buckets_respects_range() {
        let state = state().await;
        let now = now_ts();
        insert_consumption(&state, "old-kn", "ada", "coffee", now - 60 * 24 * 60 * 60).await;
        insert_consumption(&state, "recent-kn", "ada", "coffee", now - 60).await;
        insert_consumption(&state, "recent-guest", "guest", "coffee", now - 30).await;

        let Json(window) = global_buckets(
            State(state.clone()),
            user("ada"),
            Query(RangeQuery {
                range: Some("30".into()),
            }),
        )
        .await
        .unwrap();
        let kn_total: i64 = window.buckets.iter().map(|b| b.kn_count).sum();
        let guest_total: i64 = window.buckets.iter().map(|b| b.guest_count).sum();
        assert_eq!(kn_total, 1);
        assert_eq!(guest_total, 1);

        let Json(all) = global_buckets(
            State(state),
            user("ada"),
            Query(RangeQuery {
                range: Some("all".into()),
            }),
        )
        .await
        .unwrap();
        let kn_all: i64 = all.buckets.iter().map(|b| b.kn_count).sum();
        assert_eq!(kn_all, 2);
    }

    #[tokio::test]
    async fn global_beverages_respects_range() {
        let state = state().await;
        let now = now_ts();
        insert_consumption(&state, "old-kn", "ada", "coffee", now - 60 * 24 * 60 * 60).await;
        insert_consumption(&state, "recent-kn", "ada", "coffee", now - 60).await;
        insert_consumption(&state, "recent-kn-2", "ada", "other", now - 30).await;

        let Json(window) = global_beverages(
            State(state),
            user("ada"),
            Query(RangeQuery {
                range: Some("30".into()),
            }),
        )
        .await
        .unwrap();

        assert_eq!(
            window
                .beverages
                .iter()
                .map(|summary| summary.count)
                .sum::<i64>(),
            2
        );
    }

    #[test]
    fn beverage_summaries_include_every_beverage() {
        let rows = [
            ConsumptionForStats {
                consumed_at: 1,
                beverage_id: "coffee".into(),
                beverage_name: "Coffee (Vollautomat)".into(),
                price_cents: 50,
                caffeine_mg: 90,
            },
            ConsumptionForStats {
                consumed_at: 2,
                beverage_id: "espresso_pads".into(),
                beverage_name: "Espresso (pads)".into(),
                price_cents: 50,
                caffeine_mg: 90,
            },
            ConsumptionForStats {
                consumed_at: 3,
                beverage_id: "eis".into(),
                beverage_name: "Eis".into(),
                price_cents: 50,
                caffeine_mg: 0,
            },
        ];

        let summaries = beverage_summaries(&rows);
        assert_eq!(
            summaries
                .iter()
                .map(|summary| summary.beverage_id.as_str())
                .collect::<Vec<_>>(),
            ["coffee", "eis", "espresso_pads"]
        );
    }

    #[test]
    fn personal_range_filter_defaults_to_all_time() {
        let now = now_ts();
        let rows = || {
            vec![
                ConsumptionForStats {
                    consumed_at: now - 60 * 24 * 60 * 60,
                    beverage_id: "coffee".into(),
                    beverage_name: "Coffee (Vollautomat)".into(),
                    price_cents: 50,
                    caffeine_mg: 90,
                },
                ConsumptionForStats {
                    consumed_at: now - 60 * 60,
                    beverage_id: "coffee".into(),
                    beverage_name: "Coffee (Vollautomat)".into(),
                    price_cents: 50,
                    caffeine_mg: 90,
                },
            ]
        };

        assert_eq!(filter_by_range(rows(), None).len(), 2);
        assert_eq!(filter_by_range(rows(), Some("all")).len(), 2);
        assert_eq!(filter_by_range(rows(), Some("30")).len(), 1);
    }
}
