//! Lucky wheel ("Glücksrad").
//!
//! Members spin a prize wheel. Every member gets one free welcome spin; after
//! that a spin costs [`SPIN_PRICE_CENTS`], booked onto the open tab like a
//! drink, and at most [`MAX_PAID_SPINS_PER_WEEK`] paid spins fit into one
//! local ISO week. Re-spin vouchers grant another free spin.
//!
//! Prizes cost the Kaffeekasse nothing except the free drink, so the odds in
//! [`PRIZES`] keep the expected payout far below the price (see the
//! `house_always_profits` test). Every draw stays in `wheel_spins`, which is
//! also the source for the staff statistics.

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    AppState,
    achievements::{AchievementEvent, evaluate_global},
    auth::CurrentUser,
    cleaning::week_key,
    cosmetics::{catalog, is_member, sync_grants},
    db::now_ts,
    error::{ApiError, ApiResult},
    notifications::notify_unlocks,
    stats::resolve_tz,
};

/// Price of one paid spin.
pub const SPIN_PRICE_CENTS: i64 = 25;
/// Paid spins per user and local ISO week. Free spins don't count.
pub const MAX_PAID_SPINS_PER_WEEK: i64 = 5;
/// How many recent draws `GET /wheel/me` returns.
const RECENT_SPINS: i64 = 10;
/// Weeks shown in the staff statistics.
const STATS_WEEKS: i64 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
pub enum WheelPrize {
    #[serde(rename = "nothing")]
    Nothing,
    /// "Oh noooo… anyway": a Snaggs picture to look at, nothing more.
    #[serde(rename = "oh_no")]
    OhNo,
    #[serde(rename = "crate_1")]
    Crate1,
    #[serde(rename = "crate_2")]
    Crate2,
    #[serde(rename = "crate_3")]
    Crate3,
    /// The next priced drink is free.
    #[serde(rename = "free_drink")]
    FreeDrink,
    /// Another spin, on the house.
    #[serde(rename = "respin")]
    Respin,
}

impl WheelPrize {
    pub const ALL: [WheelPrize; 7] = [
        WheelPrize::Nothing,
        WheelPrize::OhNo,
        WheelPrize::Crate1,
        WheelPrize::Crate2,
        WheelPrize::Crate3,
        WheelPrize::FreeDrink,
        WheelPrize::Respin,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            WheelPrize::Nothing => "nothing",
            WheelPrize::OhNo => "oh_no",
            WheelPrize::Crate1 => "crate_1",
            WheelPrize::Crate2 => "crate_2",
            WheelPrize::Crate3 => "crate_3",
            WheelPrize::FreeDrink => "free_drink",
            WheelPrize::Respin => "respin",
        }
    }

    pub fn parse(value: &str) -> Option<WheelPrize> {
        WheelPrize::ALL
            .into_iter()
            .find(|prize| prize.as_str() == value)
    }

    pub fn crates(self) -> i64 {
        match self {
            WheelPrize::Crate1 => 1,
            WheelPrize::Crate2 => 2,
            WheelPrize::Crate3 => 3,
            _ => 0,
        }
    }

    pub fn weight(self) -> u64 {
        PRIZES
            .iter()
            .find(|(prize, _)| *prize == self)
            .map(|(_, weight)| *weight)
            .unwrap_or(0)
    }
}

/// Prize odds as weights (they sum to [`TOTAL_WEIGHT`] = 100, i.e. percent).
pub const PRIZES: [(WheelPrize, u64); 7] = [
    (WheelPrize::Nothing, 38),
    (WheelPrize::OhNo, 20),
    (WheelPrize::Crate1, 15),
    (WheelPrize::Crate2, 7),
    (WheelPrize::Crate3, 3),
    (WheelPrize::FreeDrink, 7),
    (WheelPrize::Respin, 10),
];
pub const TOTAL_WEIGHT: u64 = 100;

/// Snaggs pictures shown for the "Oh noooo… anyway" prize (private artwork).
pub const SNAGGS_ARTWORK: [&str; 6] = [
    "avatar_snaggs_brb",
    "avatar_snaggs_oh_no",
    "avatar_snaggs_smooth",
    "avatar_snaggs_party",
    "avatar_snaggs_help",
    "avatar_snaggs_monday",
];

/// Picks a prize by weight from a uniform random number.
pub fn roll(value: u64) -> WheelPrize {
    let mut remaining = value % TOTAL_WEIGHT;
    for (prize, weight) in PRIZES {
        if remaining < weight {
            return prize;
        }
        remaining -= weight;
    }
    WheelPrize::Nothing
}

fn probability(prize: WheelPrize) -> f64 {
    prize.weight() as f64 / TOTAL_WEIGHT as f64
}

/// Expected cost to the Kaffeekasse of one paid spin, in cents, when a free
/// drink is worth `drink_value_cents`. Re-spins chain further draws, so the
/// payout of a single draw is scaled by the geometric series `1 / (1 - p)`.
pub fn expected_payout_cents(drink_value_cents: f64) -> f64 {
    probability(WheelPrize::FreeDrink) * drink_value_cents / (1.0 - probability(WheelPrize::Respin))
}

fn artwork_path(artwork_id: Option<&str>) -> Option<String> {
    artwork_id
        .and_then(catalog::find)
        .and_then(|item| item.artwork_path())
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/wheel/me", get(my_wheel))
        .route("/wheel/spin", post(spin_wheel))
        .route("/wheel/stats", get(wheel_stats))
}

#[derive(Serialize, ToSchema)]
pub struct WheelOdds {
    pub prize: WheelPrize,
    pub weight: u64,
    /// Probability of this prize per draw (0..1).
    pub probability: f64,
}

#[derive(Serialize, ToSchema)]
pub struct WheelSpin {
    pub id: String,
    pub prize: WheelPrize,
    /// `welcome`, `voucher` or `paid`.
    pub source: String,
    pub cost_cents: i64,
    pub spun_at: i64,
    /// Picture of an "Oh noooo… anyway" draw (members-only artwork).
    pub artwork_path: Option<String>,
    /// For a free drink: whether it has been used on a logged drink yet.
    pub redeemed: bool,
}

#[derive(Serialize, ToSchema)]
pub struct WheelStatus {
    pub price_cents: i64,
    /// Free spins waiting (welcome spin plus unused re-spin vouchers).
    pub free_spins_available: i64,
    pub welcome_spin_available: bool,
    /// Current local ISO week, e.g. `2026-W45`.
    pub week: String,
    pub paid_spins_this_week: i64,
    pub paid_spins_limit: i64,
    /// Won free drinks that the next logged drink will use.
    pub free_drinks_available: i64,
    pub total_spins: i64,
    pub odds: Vec<WheelOdds>,
    pub recent_spins: Vec<WheelSpin>,
}

#[derive(Deserialize, ToSchema)]
pub struct WheelSpinRequest {
    /// `false` uses a free spin; `true` buys one (booked onto the tab). A paid
    /// spin is refused while free spins are still waiting.
    pub paid: bool,
    /// Client-chosen UUID for this spin. Sending the same id again (a retry
    /// after a lost response) returns the recorded spin instead of booking
    /// and charging a new one.
    #[serde(default)]
    pub spin_id: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct WheelSpinResponse {
    pub spin: WheelSpin,
    pub crates_granted: i64,
    pub unlocked_achievements: Vec<AchievementEvent>,
    pub status: WheelStatus,
}

#[derive(Serialize, ToSchema)]
pub struct WheelPrizeStat {
    pub prize: WheelPrize,
    pub count: i64,
    /// Observed share of all draws (0..1).
    pub share: f64,
    /// Share the odds promise (0..1).
    pub expected_share: f64,
}

#[derive(Serialize, ToSchema)]
pub struct WheelWeekStat {
    pub week: String,
    pub spins: i64,
    pub paid_spins: i64,
    pub revenue_cents: i64,
}

#[derive(Serialize, ToSchema)]
pub struct WheelStats {
    pub price_cents: i64,
    pub total_spins: i64,
    pub paid_spins: i64,
    pub welcome_spins: i64,
    pub voucher_spins: i64,
    /// Distinct members who spun at least once.
    pub players: i64,
    /// Sum of all paid spins (booked onto tabs).
    pub revenue_cents: i64,
    pub free_drinks_won: i64,
    pub free_drinks_redeemed: i64,
    /// Price of the drinks that were made free by the wheel.
    pub free_drinks_value_cents: i64,
    pub crates_granted: i64,
    /// Revenue minus the value of redeemed free drinks.
    pub net_cents: i64,
    /// Expected payout per paid spin with a 50 ct drink, in cents.
    pub expected_payout_cents: f64,
    pub prizes: Vec<WheelPrizeStat>,
    /// Most recent weeks with draws, oldest first.
    pub weeks: Vec<WheelWeekStat>,
}

fn ensure_member(user: &CurrentUser) -> ApiResult<()> {
    if is_member(user) {
        Ok(())
    } else {
        Err(ApiError::Forbidden)
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/wheel/me",
    responses(
        (status = 200, body = WheelStatus),
        (status = 403, description = "Members only")
    )
)]
pub(crate) async fn my_wheel(
    State(state): State<AppState>,
    user: CurrentUser,
) -> ApiResult<Json<WheelStatus>> {
    ensure_member(&user)?;
    Ok(Json(load_status(&state, &user.id, now_ts()).await?))
}

#[utoipa::path(
    post,
    path = "/api/v1/wheel/spin",
    request_body = WheelSpinRequest,
    responses(
        (status = 201, body = WheelSpinResponse),
        (status = 200, body = WheelSpinResponse, description = "Replay of an already recorded `spin_id`"),
        (status = 400, description = "`spin_id` is not a UUID"),
        (status = 403, description = "Members only"),
        (status = 409, description = "No free spin left, free spins still waiting, or weekly limit reached")
    )
)]
pub(crate) async fn spin_wheel(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(input): Json<WheelSpinRequest>,
) -> ApiResult<(StatusCode, Json<WheelSpinResponse>)> {
    ensure_member(&user)?;
    let id = match &input.spin_id {
        Some(raw) => Uuid::parse_str(raw)
            .map_err(|_| ApiError::BadRequest("spin_id must be a UUID".into()))?
            .to_string(),
        None => Uuid::new_v4().to_string(),
    };
    let now = now_ts();
    if let Some(replay) = replay_spin(&state, &user, &id, now).await? {
        return Ok((StatusCode::OK, Json(replay)));
    }

    let prize = roll(catalog::random_u64());
    let artwork_id = (prize == WheelPrize::OhNo)
        .then(|| SNAGGS_ARTWORK[(catalog::random_u64() % SNAGGS_ARTWORK.len() as u64) as usize]);
    let spin = match record_spin(&state, &user, &id, input.paid, prize, artwork_id, now).await {
        Ok(spin) => spin,
        // A concurrent retry with the same id won the insert: answer with its spin.
        Err(error) => match replay_spin(&state, &user, &id, now).await? {
            Some(replay) => return Ok((StatusCode::OK, Json(replay))),
            None => return Err(error),
        },
    };

    // The spin (and its charge) is committed now. Nothing after this point may
    // turn the request into an error the user would answer with another paid spin.
    let mut unlocked_achievements = match evaluate_global(&state, &user.id, now).await {
        Ok(unlocks) => unlocks,
        Err(error) => {
            tracing::warn!(?error, "achievement check after a wheel spin failed");
            Vec::new()
        }
    };
    if !unlocked_achievements.is_empty() {
        if let Err(error) = sync_grants(&state, &user.id).await {
            tracing::warn!(?error, "crate sync after a wheel spin failed");
        }
        notify_unlocks(&state, &user, &unlocked_achievements).await;
    }
    unlocked_achievements.sort_by(|a, b| a.id.cmp(&b.id));

    Ok((
        StatusCode::CREATED,
        Json(WheelSpinResponse {
            crates_granted: prize.crates(),
            spin,
            unlocked_achievements,
            status: load_status(&state, &user.id, now).await?,
        }),
    ))
}

// Parameters of every spin insert: ?1 user, ?2 week, ?3 weekly limit, ?4 id,
// ?5 now, ?6 prize, ?7 artwork. The guards share the insert statement, so the
// check and the write cannot interleave with a concurrent spin.
const INSERT_WELCOME: &str = "INSERT INTO wheel_spins
     (id, user_id, spun_at, week_key, cost_cents, source, prize, artwork_id)
     SELECT ?4, ?1, ?5, ?2, 0, 'welcome', ?6, ?7
     WHERE NOT EXISTS (SELECT 1 FROM wheel_spins WHERE user_id = ?1 AND source = 'welcome')";
const INSERT_VOUCHER: &str = "INSERT INTO wheel_spins
     (id, user_id, spun_at, week_key, cost_cents, source, prize, artwork_id)
     SELECT ?4, ?1, ?5, ?2, 0, 'voucher', ?6, ?7
     WHERE (SELECT COUNT(*) FROM wheel_spins WHERE user_id = ?1 AND prize = 'respin')
         > (SELECT COUNT(*) FROM wheel_spins WHERE user_id = ?1 AND source = 'voucher')";
/// Paid spins need every free spin used up and room in this week's limit.
const INSERT_PAID: &str = "INSERT INTO wheel_spins
     (id, user_id, spun_at, week_key, cost_cents, source, prize, artwork_id)
     SELECT ?4, ?1, ?5, ?2, 25, 'paid', ?6, ?7
     WHERE EXISTS (SELECT 1 FROM wheel_spins WHERE user_id = ?1 AND source = 'welcome')
       AND (SELECT COUNT(*) FROM wheel_spins WHERE user_id = ?1 AND prize = 'respin')
           <= (SELECT COUNT(*) FROM wheel_spins WHERE user_id = ?1 AND source = 'voucher')
       AND (SELECT COUNT(*) FROM wheel_spins
            WHERE user_id = ?1 AND source = 'paid' AND week_key = ?2) < ?3";

/// The response for a spin that is already recorded under `id`, if any.
async fn replay_spin(
    state: &AppState,
    user: &CurrentUser,
    id: &str,
    now: i64,
) -> ApiResult<Option<WheelSpinResponse>> {
    let owner: Option<(String,)> = sqlx::query_as("SELECT user_id FROM wheel_spins WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.db)
        .await?;
    match owner {
        None => return Ok(None),
        Some((owner,)) if owner != user.id => {
            return Err(ApiError::Conflict("spin_id already used".into()));
        }
        Some(_) => {}
    }
    let row = sqlx::query_as::<_, SpinRow>(
        "SELECT id, prize, source, cost_cents, spun_at, artwork_id, redeemed_consumption_id
         FROM wheel_spins WHERE id = ?",
    )
    .bind(id)
    .fetch_one(&state.db)
    .await?;
    let spin = row.into_spin();
    Ok(Some(WheelSpinResponse {
        crates_granted: spin.prize.crates(),
        spin,
        unlocked_achievements: Vec::new(),
        status: load_status(state, &user.id, now).await?,
    }))
}

/// Books one draw with an already rolled prize. Every guard (free spin left,
/// weekly limit) sits in the same statement as the insert, so concurrent
/// requests cannot overdraw.
async fn record_spin(
    state: &AppState,
    user: &CurrentUser,
    id: &str,
    paid: bool,
    prize: WheelPrize,
    artwork_id: Option<&str>,
    now: i64,
) -> ApiResult<WheelSpin> {
    let tz = resolve_tz(&state.config.timezone);
    let week = week_key(now, &tz);
    let id = id.to_string();

    let mut tx = state.db.begin().await?;
    let attempts: &[(&str, i64, &'static str)] = if paid {
        &[("paid", SPIN_PRICE_CENTS, INSERT_PAID)]
    } else {
        &[
            ("welcome", 0, INSERT_WELCOME),
            ("voucher", 0, INSERT_VOUCHER),
        ]
    };

    let mut source = None;
    for (kind, cost, sql) in attempts {
        let result = sqlx::query(*sql)
            .bind(&user.id)
            .bind(&week)
            .bind(MAX_PAID_SPINS_PER_WEEK)
            .bind(&id)
            .bind(now)
            .bind(prize.as_str())
            .bind(artwork_id)
            .execute(&mut *tx)
            .await?;
        if result.rows_affected() > 0 {
            source = Some((kind.to_string(), *cost));
            break;
        }
    }

    let Some((source, cost_cents)) = source else {
        drop(tx);
        return Err(if paid {
            let status = load_status(state, &user.id, now).await?;
            if status.free_spins_available > 0 {
                ApiError::Conflict("use your free spin first".into())
            } else {
                ApiError::Conflict(format!(
                    "you already bought {MAX_PAID_SPINS_PER_WEEK} spins this week"
                ))
            }
        } else {
            ApiError::Conflict("no free spin left".into())
        });
    };

    for index in 1..=prize.crates() {
        sqlx::query(
            "INSERT OR IGNORE INTO lootbox_grants (id, user_id, source, source_ref, granted_at)
             VALUES (?, ?, 'wheel', ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(&user.id)
        .bind(format!("{id}#{index}"))
        .bind(now)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    Ok(WheelSpin {
        id,
        prize,
        source,
        cost_cents,
        spun_at: now,
        artwork_path: artwork_path(artwork_id),
        redeemed: false,
    })
}

async fn count(state: &AppState, sql: &'static str, user_id: &str) -> ApiResult<i64> {
    let (value,): (i64,) = sqlx::query_as(sql)
        .bind(user_id)
        .fetch_one(&state.db)
        .await?;
    Ok(value)
}

#[derive(sqlx::FromRow)]
struct SpinRow {
    id: String,
    prize: String,
    source: String,
    cost_cents: i64,
    spun_at: i64,
    artwork_id: Option<String>,
    redeemed_consumption_id: Option<String>,
}

impl SpinRow {
    fn into_spin(self) -> WheelSpin {
        WheelSpin {
            prize: WheelPrize::parse(&self.prize).unwrap_or(WheelPrize::Nothing),
            artwork_path: artwork_path(self.artwork_id.as_deref()),
            redeemed: self.redeemed_consumption_id.is_some(),
            id: self.id,
            source: self.source,
            cost_cents: self.cost_cents,
            spun_at: self.spun_at,
        }
    }
}

pub async fn load_status(state: &AppState, user_id: &str, now: i64) -> ApiResult<WheelStatus> {
    let tz = resolve_tz(&state.config.timezone);
    let week = week_key(now, &tz);
    let welcome_used = count(
        state,
        "SELECT COUNT(*) FROM wheel_spins WHERE user_id = ? AND source = 'welcome'",
        user_id,
    )
    .await?
        > 0;
    let (vouchers_left,): (i64,) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM wheel_spins WHERE user_id = ?1 AND prize = 'respin')
              - (SELECT COUNT(*) FROM wheel_spins WHERE user_id = ?1 AND source = 'voucher')",
    )
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;
    let (paid_spins_this_week,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM wheel_spins WHERE user_id = ? AND source = 'paid' AND week_key = ?",
    )
    .bind(user_id)
    .bind(&week)
    .fetch_one(&state.db)
    .await?;
    let free_drinks_available = free_drinks_available(state, user_id).await?;
    let total_spins = count(
        state,
        "SELECT COUNT(*) FROM wheel_spins WHERE user_id = ?",
        user_id,
    )
    .await?;
    let recent_spins = sqlx::query_as::<_, SpinRow>(
        "SELECT id, prize, source, cost_cents, spun_at, artwork_id, redeemed_consumption_id
         FROM wheel_spins WHERE user_id = ?
         ORDER BY spun_at DESC, rowid DESC
         LIMIT ?",
    )
    .bind(user_id)
    .bind(RECENT_SPINS)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(SpinRow::into_spin)
    .collect();

    Ok(WheelStatus {
        price_cents: SPIN_PRICE_CENTS,
        free_spins_available: i64::from(!welcome_used) + vouchers_left.max(0),
        welcome_spin_available: !welcome_used,
        week,
        paid_spins_this_week,
        paid_spins_limit: MAX_PAID_SPINS_PER_WEEK,
        free_drinks_available,
        total_spins,
        odds: PRIZES
            .iter()
            .map(|(prize, weight)| WheelOdds {
                prize: *prize,
                weight: *weight,
                probability: probability(*prize),
            })
            .collect(),
        recent_spins,
    })
}

pub async fn free_drinks_available(state: &AppState, user_id: &str) -> ApiResult<i64> {
    count(
        state,
        "SELECT COUNT(*) FROM wheel_spins
         WHERE user_id = ? AND prize = 'free_drink' AND redeemed_consumption_id IS NULL",
        user_id,
    )
    .await
}

/// Uses the user's oldest unused free drink on `consumption_id` when the
/// beverage has a price. Returns whether a voucher was used. The drink then
/// costs nothing and takes no stamp-card position.
pub async fn redeem_free_drink(
    state: &AppState,
    user_id: &str,
    consumption_id: &str,
    beverage_id: &str,
) -> ApiResult<bool> {
    let result = sqlx::query(
        "UPDATE wheel_spins
         SET redeemed_consumption_id = ?1,
             redeemed_value_cents = (SELECT price_cents FROM beverages WHERE id = ?3)
         WHERE id = (
             SELECT id FROM wheel_spins
             WHERE user_id = ?2 AND prize = 'free_drink' AND redeemed_consumption_id IS NULL
             ORDER BY spun_at, rowid LIMIT 1
         )
         AND EXISTS (SELECT 1 FROM beverages WHERE id = ?3 AND price_cents > 0)",
    )
    .bind(consumption_id)
    .bind(user_id)
    .bind(beverage_id)
    .execute(&state.db)
    .await?;
    Ok(result.rows_affected() > 0)
}

/// Wheel charges booked in `[cutoff, until)` (the open tab).
pub async fn unpaid_wheel_cents(
    state: &AppState,
    user_id: &str,
    cutoff: i64,
    until: i64,
) -> ApiResult<i64> {
    let (amount,): (i64,) = sqlx::query_as(
        "SELECT COALESCE(SUM(cost_cents), 0) FROM wheel_spins
         WHERE user_id = ? AND spun_at >= ? AND spun_at < ?",
    )
    .bind(user_id)
    .bind(cutoff)
    .bind(until)
    .fetch_one(&state.db)
    .await?;
    Ok(amount)
}

/// Value of an achievement counter based on wheel draws, or `None` when the
/// criteria kind is not a wheel one.
pub async fn achievement_value(
    state: &AppState,
    user_id: &str,
    criteria_kind: &str,
) -> ApiResult<Option<i64>> {
    if let Some(prize) = criteria_kind.strip_prefix("wheel_prize:") {
        let (value,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM wheel_spins WHERE user_id = ? AND prize = ?")
                .bind(user_id)
                .bind(prize)
                .fetch_one(&state.db)
                .await?;
        return Ok(Some(value));
    }
    let value = match criteria_kind {
        "wheel_spins" => {
            count(
                state,
                "SELECT COUNT(*) FROM wheel_spins WHERE user_id = ?",
                user_id,
            )
            .await?
        }
        "wheel_paid_spins" => {
            count(
                state,
                "SELECT COUNT(*) FROM wheel_spins WHERE user_id = ? AND source = 'paid'",
                user_id,
            )
            .await?
        }
        "wheel_snaggs_seen" => {
            count(
                state,
                "SELECT COUNT(DISTINCT artwork_id) FROM wheel_spins
                 WHERE user_id = ? AND artwork_id IS NOT NULL",
                user_id,
            )
            .await?
        }
        "wheel_nothing_run" => {
            let prizes: Vec<(String,)> = sqlx::query_as(
                "SELECT prize FROM wheel_spins WHERE user_id = ? ORDER BY spun_at, rowid",
            )
            .bind(user_id)
            .fetch_all(&state.db)
            .await?;
            longest_run(prizes.iter().map(|(prize,)| prize.as_str()), "nothing")
        }
        _ => return Ok(None),
    };
    Ok(Some(value))
}

pub fn is_wheel_criteria(criteria_kind: &str) -> bool {
    criteria_kind.starts_with("wheel_prize:")
        || matches!(
            criteria_kind,
            "wheel_spins" | "wheel_paid_spins" | "wheel_snaggs_seen" | "wheel_nothing_run"
        )
}

fn longest_run<'a>(values: impl Iterator<Item = &'a str>, target: &str) -> i64 {
    let mut best = 0;
    let mut current = 0;
    for value in values {
        if value == target {
            current += 1;
            best = best.max(current);
        } else {
            current = 0;
        }
    }
    best
}

#[utoipa::path(
    get,
    path = "/api/v1/wheel/stats",
    responses(
        (status = 200, body = WheelStats),
        (status = 403, description = "Staff only")
    )
)]
pub(crate) async fn wheel_stats(
    State(state): State<AppState>,
    user: CurrentUser,
) -> ApiResult<Json<WheelStats>> {
    if !user.stock_manager {
        return Err(ApiError::Forbidden);
    }
    Ok(Json(load_stats(&state).await?))
}

pub async fn load_stats(state: &AppState) -> ApiResult<WheelStats> {
    let (total_spins, paid_spins, welcome_spins, voucher_spins, players, revenue_cents): (
        i64,
        i64,
        i64,
        i64,
        i64,
        i64,
    ) = sqlx::query_as(
        "SELECT COUNT(*),
                COALESCE(SUM(source = 'paid'), 0),
                COALESCE(SUM(source = 'welcome'), 0),
                COALESCE(SUM(source = 'voucher'), 0),
                COUNT(DISTINCT user_id),
                COALESCE(SUM(cost_cents), 0)
         FROM wheel_spins",
    )
    .fetch_one(&state.db)
    .await?;

    let (free_drinks_redeemed, free_drinks_value_cents): (i64, i64) = sqlx::query_as(
        // Valued at the price when the drink was had, not today's price.
        "SELECT COUNT(*), COALESCE(SUM(redeemed_value_cents), 0)
         FROM wheel_spins
         WHERE redeemed_consumption_id IS NOT NULL",
    )
    .fetch_one(&state.db)
    .await?;

    let prize_counts: Vec<(String, i64)> =
        sqlx::query_as("SELECT prize, COUNT(*) FROM wheel_spins GROUP BY prize")
            .fetch_all(&state.db)
            .await?;
    let count_of = |prize: WheelPrize| {
        prize_counts
            .iter()
            .find(|(name, _)| name == prize.as_str())
            .map(|(_, count)| *count)
            .unwrap_or(0)
    };
    let prizes: Vec<WheelPrizeStat> = WheelPrize::ALL
        .into_iter()
        .map(|prize| {
            let count = count_of(prize);
            WheelPrizeStat {
                prize,
                count,
                share: if total_spins > 0 {
                    count as f64 / total_spins as f64
                } else {
                    0.0
                },
                expected_share: probability(prize),
            }
        })
        .collect();
    let crates_granted = WheelPrize::ALL
        .into_iter()
        .map(|prize| count_of(prize) * prize.crates())
        .sum();

    let mut weeks: Vec<WheelWeekStat> = sqlx::query_as::<_, (String, i64, i64, i64)>(
        "SELECT week_key, COUNT(*), COALESCE(SUM(source = 'paid'), 0), COALESCE(SUM(cost_cents), 0)
         FROM wheel_spins
         GROUP BY week_key
         ORDER BY week_key DESC
         LIMIT ?",
    )
    .bind(STATS_WEEKS)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|(week, spins, paid_spins, revenue_cents)| WheelWeekStat {
        week,
        spins,
        paid_spins,
        revenue_cents,
    })
    .collect();
    weeks.reverse();

    Ok(WheelStats {
        price_cents: SPIN_PRICE_CENTS,
        total_spins,
        paid_spins,
        welcome_spins,
        voucher_spins,
        players,
        revenue_cents,
        free_drinks_won: count_of(WheelPrize::FreeDrink),
        free_drinks_redeemed,
        free_drinks_value_cents,
        crates_granted,
        net_cents: revenue_cents - free_drinks_value_cents,
        expected_payout_cents: expected_payout_cents(50.0),
        prizes,
        weeks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weights_sum_to_total() {
        let sum: u64 = PRIZES.iter().map(|(_, weight)| weight).sum();
        assert_eq!(sum, TOTAL_WEIGHT);
        assert_eq!(PRIZES.len(), WheelPrize::ALL.len());
    }

    #[test]
    fn house_always_profits() {
        // A coffee is 50 ct; even the 1 EUR "Other" drink keeps the Kasse ahead.
        let coffee = expected_payout_cents(50.0);
        assert!((coffee - 3.889).abs() < 0.01, "payout {coffee}");
        assert!(SPIN_PRICE_CENTS as f64 - coffee > 21.0);
        assert!(expected_payout_cents(100.0) < SPIN_PRICE_CENTS as f64 / 2.0);
    }

    #[test]
    fn roll_follows_the_weights() {
        let mut counts = std::collections::HashMap::new();
        for value in 0..TOTAL_WEIGHT * 10 {
            *counts.entry(roll(value)).or_insert(0u64) += 1;
        }
        for (prize, weight) in PRIZES {
            assert_eq!(counts[&prize], weight * 10, "{prize:?}");
        }
    }

    #[test]
    fn live_random_draws_match_the_odds() {
        let draws = 200_000u64;
        let mut counts = std::collections::HashMap::new();
        for _ in 0..draws {
            *counts.entry(roll(catalog::random_u64())).or_insert(0u64) += 1;
        }
        for (prize, weight) in PRIZES {
            let share = counts.get(&prize).copied().unwrap_or(0) as f64 / draws as f64;
            let expected = weight as f64 / TOTAL_WEIGHT as f64;
            assert!(
                (share - expected).abs() < 0.006,
                "{prize:?}: {share} vs {expected}"
            );
        }
    }

    #[test]
    fn prize_names_round_trip() {
        for prize in WheelPrize::ALL {
            assert_eq!(WheelPrize::parse(prize.as_str()), Some(prize));
            assert_eq!(
                serde_json::to_value(prize).unwrap(),
                serde_json::Value::String(prize.as_str().into())
            );
        }
    }

    #[test]
    fn snaggs_artwork_exists_in_the_catalog() {
        for id in SNAGGS_ARTWORK {
            let item = catalog::find(id).expect(id);
            assert!(item.private_artwork, "{id} must stay private");
            assert!(artwork_path(Some(id)).is_some());
        }
    }

    #[test]
    fn paid_insert_charges_the_spin_price() {
        assert!(INSERT_PAID.contains(&format!(", {SPIN_PRICE_CENTS}, 'paid'")));
    }

    #[test]
    fn longest_run_counts_consecutive_values() {
        let values = [
            "nothing", "nothing", "oh_no", "nothing", "nothing", "nothing",
        ];
        assert_eq!(longest_run(values.into_iter(), "nothing"), 3);
        assert_eq!(longest_run(["oh_no"].into_iter(), "nothing"), 0);
    }
}

#[cfg(test)]
mod flow_tests {
    use axum::extract::State;

    use super::*;
    use crate::{
        cosmetics::{
            load_locker,
            test_support::{member, state_with_users},
        },
        payments::{MarkPaidRequest, mark_paid, my_payments, settle_tab},
    };

    const WEEK: i64 = 7 * 24 * 60 * 60;

    async fn spin(
        state: &AppState,
        user: &str,
        paid: bool,
        prize: WheelPrize,
        now: i64,
    ) -> ApiResult<WheelSpin> {
        let artwork = (prize == WheelPrize::OhNo).then_some(SNAGGS_ARTWORK[0]);
        let id = Uuid::new_v4().to_string();
        record_spin(state, &member(user), &id, paid, prize, artwork, now).await
    }

    async fn insert_drink(state: &AppState, id: &str, user: &str, at: i64) {
        sqlx::query(
            "INSERT INTO consumptions (id, user_id, machine_id, beverage_id, consumed_at, created_at)
             VALUES (?, ?, 'main', 'coffee', ?, ?)",
        )
        .bind(id)
        .bind(user)
        .bind(at)
        .bind(at)
        .execute(&state.db)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn welcome_spin_then_paid_spins_up_to_the_weekly_limit() {
        let state = state_with_users(&["alice"]).await;
        let now = now_ts();

        // Paying is refused while the welcome spin is still waiting.
        assert!(matches!(
            spin(&state, "alice", true, WheelPrize::Nothing, now).await,
            Err(ApiError::Conflict(_))
        ));
        let welcome = spin(&state, "alice", false, WheelPrize::Nothing, now)
            .await
            .unwrap();
        assert_eq!(welcome.source, "welcome");
        assert_eq!(welcome.cost_cents, 0);
        assert!(matches!(
            spin(&state, "alice", false, WheelPrize::Nothing, now).await,
            Err(ApiError::Conflict(_))
        ));

        for _ in 0..MAX_PAID_SPINS_PER_WEEK {
            let paid = spin(&state, "alice", true, WheelPrize::Nothing, now)
                .await
                .unwrap();
            assert_eq!(paid.cost_cents, SPIN_PRICE_CENTS);
        }
        assert!(matches!(
            spin(&state, "alice", true, WheelPrize::Nothing, now).await,
            Err(ApiError::Conflict(_))
        ));
        let status = load_status(&state, "alice", now).await.unwrap();
        assert_eq!(status.paid_spins_this_week, MAX_PAID_SPINS_PER_WEEK);
        assert_eq!(status.free_spins_available, 0);
        assert_eq!(status.total_spins, 6);

        // A new week has room again.
        assert!(
            spin(&state, "alice", true, WheelPrize::Nothing, now + WEEK)
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn guests_cannot_use_the_wheel() {
        let state = state_with_users(&["guest"]).await;
        let mut guest = member("guest");
        guest.user_type = "guest".into();
        assert!(matches!(
            my_wheel(State(state.clone()), guest.clone()).await,
            Err(ApiError::Forbidden)
        ));
        assert!(matches!(
            spin_wheel(
                State(state),
                guest,
                Json(WheelSpinRequest {
                    paid: false,
                    spin_id: None
                })
            )
            .await,
            Err(ApiError::Forbidden)
        ));
    }

    #[tokio::test]
    async fn respin_vouchers_and_crates() {
        let state = state_with_users(&["alice"]).await;
        let now = now_ts();
        spin(&state, "alice", false, WheelPrize::Respin, now)
            .await
            .unwrap();
        assert_eq!(
            load_status(&state, "alice", now)
                .await
                .unwrap()
                .free_spins_available,
            1
        );
        let voucher = spin(&state, "alice", false, WheelPrize::Crate3, now)
            .await
            .unwrap();
        assert_eq!(voucher.source, "voucher");
        spin(&state, "alice", true, WheelPrize::Crate2, now)
            .await
            .unwrap();

        let locker = load_locker(&state, "alice").await.unwrap();
        assert_eq!(locker.sources.wheel, 5);
    }

    #[tokio::test]
    async fn paid_spins_land_on_the_tab_and_can_be_settled() {
        let state = state_with_users(&["alice", "staff"]).await;
        sqlx::query("UPDATE users SET stock_manager = 1 WHERE id = 'staff'")
            .execute(&state.db)
            .await
            .unwrap();
        let now = now_ts() - 10;
        spin(&state, "alice", false, WheelPrize::Nothing, now)
            .await
            .unwrap();
        spin(&state, "alice", true, WheelPrize::Nothing, now)
            .await
            .unwrap();
        spin(&state, "alice", true, WheelPrize::OhNo, now)
            .await
            .unwrap();

        let Json(tab) = my_payments(State(state.clone()), member("alice"))
            .await
            .unwrap();
        assert_eq!(tab.unpaid_amount_cents, 2 * SPIN_PRICE_CENTS);
        assert_eq!(tab.unpaid_wheel_cents, 2 * SPIN_PRICE_CENTS);
        assert_eq!(tab.unpaid_drink_count, 0);

        // A tab with only wheel spins can still be paid.
        let (_, Json(payment)) = mark_paid(
            State(state.clone()),
            member("staff"),
            Json(MarkPaidRequest {
                user_id: "alice".into(),
                received_by_user_id: "staff".into(),
                note: None,
            }),
        )
        .await
        .unwrap();
        assert_eq!(payment.amount_cents, 2 * SPIN_PRICE_CENTS);
        let Json(tab) = my_payments(State(state.clone()), member("alice"))
            .await
            .unwrap();
        assert_eq!(tab.unpaid_amount_cents, 0);
    }

    #[tokio::test]
    async fn won_free_drink_pays_for_the_next_drink_until_it_is_deleted() {
        let state = state_with_users(&["alice"]).await;
        let now = now_ts();
        spin(&state, "alice", false, WheelPrize::FreeDrink, now - 100)
            .await
            .unwrap();
        assert_eq!(free_drinks_available(&state, "alice").await.unwrap(), 1);

        insert_drink(&state, "free", "alice", now - 50).await;
        assert!(
            redeem_free_drink(&state, "alice", "free", "coffee")
                .await
                .unwrap()
        );
        insert_drink(&state, "paid", "alice", now - 10).await;
        assert!(
            !redeem_free_drink(&state, "alice", "paid", "coffee")
                .await
                .unwrap()
        );

        let Json(tab) = my_payments(State(state.clone()), member("alice"))
            .await
            .unwrap();
        assert_eq!(tab.unpaid_drink_count, 2);
        assert_eq!(tab.unpaid_amount_cents, 50);
        // The free drink takes no stamp-card position.
        let progress = crate::loyalty::card_progress_by_month(&state, "alice")
            .await
            .unwrap();
        assert_eq!(
            progress.iter().map(|month| month.paid_drinks).sum::<i64>(),
            1
        );

        // Deleting the free drink gives the voucher back.
        sqlx::query("DELETE FROM consumptions WHERE id = 'free'")
            .execute(&state.db)
            .await
            .unwrap();
        assert_eq!(free_drinks_available(&state, "alice").await.unwrap(), 1);
    }

    #[tokio::test]
    async fn wheel_achievements_unlock_and_staff_sees_statistics() {
        let state = state_with_users(&["alice", "bob"]).await;
        let (_, Json(first)) = spin_wheel(
            State(state.clone()),
            member("alice"),
            Json(WheelSpinRequest {
                paid: false,
                spin_id: None,
            }),
        )
        .await
        .unwrap();
        assert!(
            first
                .unlocked_achievements
                .iter()
                .any(|a| a.id == "wheel_first")
        );
        assert!(!first.status.welcome_spin_available);

        let now = now_ts();
        spin(&state, "bob", false, WheelPrize::Crate3, now)
            .await
            .unwrap();
        spin(&state, "bob", true, WheelPrize::FreeDrink, now)
            .await
            .unwrap();
        let unlocks = evaluate_global(&state, "bob", now).await.unwrap();
        for id in ["wheel_first", "wheel_jackpot", "wheel_on_the_house"] {
            assert!(unlocks.iter().any(|a| a.id == id), "{id}");
        }
        insert_drink(&state, "bob-free", "bob", now).await;
        redeem_free_drink(&state, "bob", "bob-free", "coffee")
            .await
            .unwrap();

        let mut not_staff = member("alice");
        not_staff.stock_manager = false;
        assert!(matches!(
            wheel_stats(State(state.clone()), not_staff).await,
            Err(ApiError::Forbidden)
        ));
        let Json(stats) = wheel_stats(State(state.clone()), member("alice"))
            .await
            .unwrap();
        assert_eq!(stats.total_spins, 3);
        assert_eq!(stats.paid_spins, 1);
        assert_eq!(stats.welcome_spins, 2);
        assert_eq!(stats.players, 2);
        assert_eq!(stats.revenue_cents, SPIN_PRICE_CENTS);
        // Alice's spin went through the handler, so its prize is random.
        let alice_free = i64::from(first.spin.prize == WheelPrize::FreeDrink);
        assert_eq!(stats.free_drinks_won, 1 + alice_free);
        assert_eq!(stats.free_drinks_redeemed, 1);
        assert_eq!(stats.free_drinks_value_cents, 50);
        assert_eq!(stats.net_cents, SPIN_PRICE_CENTS - 50);
        assert!(stats.crates_granted >= 3);
        assert_eq!(stats.weeks.len(), 1);
        assert_eq!(stats.prizes.iter().map(|p| p.count).sum::<i64>(), 3);
    }

    #[tokio::test]
    async fn retrying_with_the_same_spin_id_never_charges_twice() {
        let state = state_with_users(&["alice"]).await;
        spin(&state, "alice", false, WheelPrize::Nothing, now_ts() - 10)
            .await
            .unwrap();
        let request = || WheelSpinRequest {
            paid: true,
            spin_id: Some("5f0c8c1e-3a43-4f0e-9d6b-2f3f7f1d9a10".into()),
        };
        let (first_status, Json(first)) =
            spin_wheel(State(state.clone()), member("alice"), Json(request()))
                .await
                .unwrap();
        let (second_status, Json(second)) =
            spin_wheel(State(state.clone()), member("alice"), Json(request()))
                .await
                .unwrap();
        assert_eq!(first_status, StatusCode::CREATED);
        assert_eq!(second_status, StatusCode::OK);
        assert_eq!(first.spin.id, second.spin.id);
        assert_eq!(first.spin.prize, second.spin.prize);
        assert_eq!(second.status.paid_spins_this_week, 1);

        // Someone else's id is refused, malformed ids too.
        let state_b = state.clone();
        sqlx::query(
            "INSERT INTO users (id, user_type, display_name, leaderboard_opt_in,
              achievement_sharing_opt_out, mattermost_opt_in, created_at, updated_at)
             VALUES ('bob', 'member', 'BOB', 1, 0, 0, 1, 1)",
        )
        .execute(&state_b.db)
        .await
        .unwrap();
        assert!(matches!(
            spin_wheel(State(state_b.clone()), member("bob"), Json(request())).await,
            Err(ApiError::Conflict(_))
        ));
        let bad = WheelSpinRequest {
            paid: false,
            spin_id: Some("nope".into()),
        };
        assert!(matches!(
            spin_wheel(State(state_b), member("bob"), Json(bad)).await,
            Err(ApiError::BadRequest(_))
        ));
    }

    #[tokio::test]
    async fn concurrent_paid_spins_never_exceed_the_weekly_limit() {
        let state = state_with_users(&["alice"]).await;
        let now = now_ts();
        spin(&state, "alice", false, WheelPrize::Nothing, now)
            .await
            .unwrap();
        let attempts = (0..10).map(|_| {
            let state = state.clone();
            tokio::spawn(async move {
                let id = Uuid::new_v4().to_string();
                record_spin(
                    &state,
                    &member("alice"),
                    &id,
                    true,
                    WheelPrize::Nothing,
                    None,
                    now,
                )
                .await
            })
        });
        let mut ok = 0;
        for attempt in attempts {
            // Losers either see the limit (Conflict) or a busy database; never a 6th spin.
            if attempt.await.unwrap().is_ok() {
                ok += 1;
            }
        }
        let status = load_status(&state, "alice", now).await.unwrap();
        assert_eq!(status.paid_spins_this_week, ok);
        assert!(ok <= MAX_PAID_SPINS_PER_WEEK, "{ok} paid spins");
        assert!(ok >= 1);
    }

    #[tokio::test]
    async fn voucher_drink_and_the_stamp_card_freebie_stack() {
        let state = state_with_users(&["alice"]).await;
        // A fixed mid-month morning: all drinks share one stamp-card month
        // whatever day the tests run on.
        let start = {
            use chrono::TimeZone;
            chrono_tz::Europe::Berlin
                .with_ymd_and_hms(2026, 10, 14, 8, 0, 0)
                .unwrap()
                .timestamp()
        };
        spin(&state, "alice", false, WheelPrize::FreeDrink, start - 10)
            .await
            .unwrap();
        // 14 coffees: one paid by the wheel voucher, the other 13 fill a card
        // and include the stamp-card freebie.
        for index in 0..14 {
            let id = format!("drink-{index}");
            insert_drink(&state, &id, "alice", start + index * 60).await;
            if index == 0 {
                assert!(
                    redeem_free_drink(&state, "alice", &id, "coffee")
                        .await
                        .unwrap()
                );
            }
        }
        let Json(tab) = my_payments(State(state.clone()), member("alice"))
            .await
            .unwrap();
        assert_eq!(tab.unpaid_drink_count, 14);
        assert_eq!(tab.unpaid_amount_cents, 12 * 50);
    }

    #[tokio::test]
    async fn a_spin_in_the_payment_second_lands_on_the_next_tab_once() {
        let state = state_with_users(&["alice", "staff"]).await;
        sqlx::query("UPDATE users SET stock_manager = 1 WHERE id = 'staff'")
            .execute(&state.db)
            .await
            .unwrap();
        // The payment is pinned to second `paid_at` (no wall clock involved,
        // so a slow CI machine can't shift the boundary).
        let paid_at = now_ts() - 100;
        spin(&state, "alice", false, WheelPrize::Nothing, paid_at - 20)
            .await
            .unwrap();
        spin(&state, "alice", true, WheelPrize::Nothing, paid_at - 10)
            .await
            .unwrap();
        // Booked in the very second the tab gets settled.
        spin(&state, "alice", true, WheelPrize::Nothing, paid_at)
            .await
            .unwrap();
        let (_, Json(payment)) = settle_tab(
            state.clone(),
            member("staff"),
            MarkPaidRequest {
                user_id: "alice".into(),
                received_by_user_id: "staff".into(),
                note: None,
            },
            || paid_at,
        )
        .await
        .unwrap();
        assert_eq!(payment.period_end_ts, paid_at);
        // Only the earlier spin is billed now ...
        assert_eq!(payment.amount_cents, SPIN_PRICE_CENTS);
        let Json(tab) = my_payments(State(state.clone()), member("alice"))
            .await
            .unwrap();
        // ... and the same-second one exactly once, on the next tab.
        assert_eq!(tab.unpaid_wheel_cents, SPIN_PRICE_CENTS);
        assert_eq!(tab.unpaid_amount_cents, SPIN_PRICE_CENTS);
    }

    #[tokio::test]
    async fn global_summary_counts_open_wheel_spins() {
        let state = state_with_users(&["alice"]).await;
        let now = now_ts();
        spin(&state, "alice", false, WheelPrize::Nothing, now)
            .await
            .unwrap();
        spin(&state, "alice", true, WheelPrize::Nothing, now)
            .await
            .unwrap();
        let Json(summary) = crate::stats::global_summary(State(state.clone()), member("alice"))
            .await
            .unwrap();
        assert_eq!(summary.unpaid_total_cents, SPIN_PRICE_CENTS);
        assert_eq!(summary.wheel_spent_cents, SPIN_PRICE_CENTS);
    }

    #[tokio::test]
    async fn three_blanks_in_a_row_unlock_not_my_day() {
        let state = state_with_users(&["alice"]).await;
        let now = now_ts();
        spin(&state, "alice", false, WheelPrize::Nothing, now)
            .await
            .unwrap();
        spin(&state, "alice", true, WheelPrize::Nothing, now)
            .await
            .unwrap();
        let unlocks = evaluate_global(&state, "alice", now).await.unwrap();
        assert!(!unlocks.iter().any(|a| a.id == "wheel_unlucky"));
        spin(&state, "alice", true, WheelPrize::Nothing, now)
            .await
            .unwrap();
        let unlocks = evaluate_global(&state, "alice", now).await.unwrap();
        assert!(unlocks.iter().any(|a| a.id == "wheel_unlucky"));
    }
}
