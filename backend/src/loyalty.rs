use axum::{
    Json, Router,
    extract::{Query, State},
    routing::get,
};
use chrono::{Datelike, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{
    AppState,
    auth::CurrentUser,
    db::now_ts,
    error::{ApiError, ApiResult},
    stats::resolve_tz,
};

/// Stamps needed to fill one card. The drink right after a full card is free,
/// so a full cycle is `STAMPS_PER_CARD + 1` paid drinks.
pub const STAMPS_PER_CARD: i64 = 12;

const CYCLE: i64 = STAMPS_PER_CARD + 1;

pub fn router() -> Router<AppState> {
    Router::new().route("/stempelkarte/me", get(my_stamp_card))
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PaidDrink {
    pub consumed_at: i64,
    pub beverage_id: String,
    pub beverage_name: String,
    pub price_cents: i64,
}

#[derive(Serialize, ToSchema)]
pub struct Stamp {
    pub consumed_at: i64,
    pub beverage_id: String,
    pub beverage_name: String,
    pub price_cents: i64,
    /// 1-based position within the month's sequence of paid drinks.
    pub position: i64,
}

#[derive(Serialize, ToSchema)]
pub struct StampCard {
    /// 1-based index of this card within the month.
    pub index: i64,
    pub complete: bool,
    pub stamps: Vec<Stamp>,
    /// The free drink earned by this card, once it has actually been consumed.
    pub reward: Option<Stamp>,
}

#[derive(Serialize, ToSchema)]
pub struct StampCardResponse {
    /// Calendar month of the card, formatted as `YYYY-MM`.
    pub month: String,
    pub month_start_ts: i64,
    pub month_end_ts: i64,
    pub stamps_per_card: i64,
    pub paid_drink_count: i64,
    pub free_drinks_earned: i64,
    pub saved_cents: i64,
    pub cards: Vec<StampCard>,
}

#[derive(Deserialize, IntoParams)]
pub struct MonthQuery {
    /// Calendar month to display, formatted as `YYYY-MM`. Defaults to the current month.
    pub month: Option<String>,
}

/// A 1-based position in the month's paid-drink sequence is free when it lands
/// right after a full card.
pub fn is_free_position(position: i64) -> bool {
    position > 0 && position % CYCLE == 0
}

/// Stamps sit on the card of their cycle; free drinks belong to the card they redeem.
fn card_index_for(position: i64) -> i64 {
    (position - 1) / CYCLE + 1
}

pub fn local_month(timestamp: i64, tz: &Tz) -> (i32, u32) {
    let date = Utc
        .timestamp_opt(timestamp, 0)
        .single()
        .map(|dt| dt.with_timezone(tz).date_naive())
        .unwrap_or_else(|| Utc::now().with_timezone(tz).date_naive());
    (date.year(), date.month())
}

fn start_of_local_day(date: NaiveDate, tz: &Tz) -> Option<i64> {
    date.and_hms_opt(0, 0, 0)
        .and_then(|naive| tz.from_local_datetime(&naive).single())
        .map(|dt| dt.timestamp())
}

/// Inclusive start and exclusive end timestamp of a calendar month in `tz`.
pub fn month_bounds(tz: &Tz, year: i32, month: u32) -> ApiResult<(i64, i64)> {
    let start_date = NaiveDate::from_ymd_opt(year, month, 1)
        .ok_or_else(|| ApiError::BadRequest("invalid month".into()))?;
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let end_date = NaiveDate::from_ymd_opt(next_year, next_month, 1)
        .ok_or_else(|| ApiError::BadRequest("invalid month".into()))?;

    let start = start_of_local_day(start_date, tz)
        .ok_or_else(|| ApiError::BadRequest("invalid month".into()))?;
    let end = start_of_local_day(end_date, tz)
        .ok_or_else(|| ApiError::BadRequest("invalid month".into()))?;
    Ok((start, end))
}

fn parse_month(value: &str) -> ApiResult<(i32, u32)> {
    let (year, month) = value
        .split_once('-')
        .ok_or_else(|| ApiError::BadRequest("month must be formatted as YYYY-MM".into()))?;
    let year: i32 = year
        .parse()
        .map_err(|_| ApiError::BadRequest("month must be formatted as YYYY-MM".into()))?;
    let month: u32 = month
        .parse()
        .map_err(|_| ApiError::BadRequest("month must be formatted as YYYY-MM".into()))?;
    if !(1..=12).contains(&month) {
        return Err(ApiError::BadRequest(
            "month must be between 01 and 12".into(),
        ));
    }
    Ok((year, month))
}

fn format_month(year: i32, month: u32) -> String {
    format!("{year:04}-{month:02}")
}

/// Splits one month's paid drinks (ordered by `consumed_at`) into stamp cards.
pub fn build_cards(drinks: &[PaidDrink]) -> Vec<StampCard> {
    let mut cards: Vec<StampCard> = Vec::new();

    for (offset, drink) in drinks.iter().enumerate() {
        let position = offset as i64 + 1;
        let index = card_index_for(position);
        while cards.len() < index as usize {
            cards.push(StampCard {
                index: cards.len() as i64 + 1,
                complete: false,
                stamps: Vec::new(),
                reward: None,
            });
        }
        let card = &mut cards[index as usize - 1];
        let stamp = Stamp {
            consumed_at: drink.consumed_at,
            beverage_id: drink.beverage_id.clone(),
            beverage_name: drink.beverage_name.clone(),
            price_cents: drink.price_cents,
            position,
        };
        if is_free_position(position) {
            card.reward = Some(stamp);
        } else {
            card.stamps.push(stamp);
            card.complete = card.stamps.len() as i64 == STAMPS_PER_CARD;
        }
    }

    if cards.is_empty() {
        cards.push(StampCard {
            index: 1,
            complete: false,
            stamps: Vec::new(),
            reward: None,
        });
    }

    cards
}

/// Open tab for consumptions at or after `cutoff`, with free drinks deducted.
/// `drinks` must contain every paid drink of each month the window touches,
/// ordered by `consumed_at` ascending, so positions are counted from the 1st.
pub fn discounted_amount_cents(drinks: &[PaidDrink], cutoff: i64, tz: &Tz) -> i64 {
    let mut position_in_month = 0i64;
    let mut current_month: Option<(i32, u32)> = None;
    let mut total = 0i64;

    for drink in drinks {
        let month = local_month(drink.consumed_at, tz);
        if current_month != Some(month) {
            current_month = Some(month);
            position_in_month = 0;
        }
        position_in_month += 1;

        if drink.consumed_at >= cutoff && !is_free_position(position_in_month) {
            total += drink.price_cents;
        }
    }

    total
}

async fn paid_drinks_between(
    state: &AppState,
    user_id: &str,
    start: i64,
    end: i64,
) -> ApiResult<Vec<PaidDrink>> {
    sqlx::query_as::<_, PaidDrink>(
        "SELECT c.consumed_at, c.beverage_id, b.name AS beverage_name, b.price_cents
         FROM consumptions c
         JOIN beverages b ON b.id = c.beverage_id
         WHERE c.user_id = ? AND b.price_cents > 0 AND c.consumed_at >= ? AND c.consumed_at < ?
         ORDER BY c.consumed_at ASC",
    )
    .bind(user_id)
    .bind(start)
    .bind(end)
    .fetch_all(&state.db)
    .await
    .map_err(ApiError::from)
}

/// Every paid drink from the start of the month containing `cutoff` onwards.
pub async fn paid_drinks_since_month_of(
    state: &AppState,
    user_id: &str,
    cutoff: i64,
) -> ApiResult<Vec<PaidDrink>> {
    let tz = resolve_tz(&state.config.timezone);
    let (year, month) = local_month(cutoff, &tz);
    let (start, _) = month_bounds(&tz, year, month)?;
    sqlx::query_as::<_, PaidDrink>(
        "SELECT c.consumed_at, c.beverage_id, b.name AS beverage_name, b.price_cents
         FROM consumptions c
         JOIN beverages b ON b.id = c.beverage_id
         WHERE c.user_id = ? AND b.price_cents > 0 AND c.consumed_at >= ?
         ORDER BY c.consumed_at ASC",
    )
    .bind(user_id)
    .bind(start)
    .fetch_all(&state.db)
    .await
    .map_err(ApiError::from)
}

#[utoipa::path(
    get,
    path = "/api/v1/stempelkarte/me",
    params(MonthQuery),
    responses((status = 200, body = StampCardResponse))
)]
pub(crate) async fn my_stamp_card(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(query): Query<MonthQuery>,
) -> ApiResult<Json<StampCardResponse>> {
    let tz = resolve_tz(&state.config.timezone);
    let (year, month) = match query.month.as_deref() {
        Some(value) => parse_month(value)?,
        None => local_month(now_ts(), &tz),
    };
    let (start, end) = month_bounds(&tz, year, month)?;

    let drinks = paid_drinks_between(&state, &user.id, start, end).await?;
    let paid_drink_count = drinks.len() as i64;
    let cards = build_cards(&drinks);
    let free_drinks_earned = cards.iter().filter(|card| card.reward.is_some()).count() as i64;
    let saved_cents = cards
        .iter()
        .filter_map(|card| card.reward.as_ref())
        .map(|reward| reward.price_cents)
        .sum();

    Ok(Json(StampCardResponse {
        month: format_month(year, month),
        month_start_ts: start,
        month_end_ts: end,
        stamps_per_card: STAMPS_PER_CARD,
        paid_drink_count,
        free_drinks_earned,
        saved_cents,
        cards,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drink(consumed_at: i64, price_cents: i64) -> PaidDrink {
        PaidDrink {
            consumed_at,
            beverage_id: "coffee".into(),
            beverage_name: "Coffee".into(),
            price_cents,
        }
    }

    fn ts(tz: &Tz, year: i32, month: u32, day: u32, hour: u32) -> i64 {
        let naive = NaiveDate::from_ymd_opt(year, month, day)
            .unwrap()
            .and_hms_opt(hour, 0, 0)
            .unwrap();
        tz.from_local_datetime(&naive).single().unwrap().timestamp()
    }

    #[test]
    fn thirteenth_drink_is_free_and_card_restarts() {
        assert!(!is_free_position(1));
        assert!(!is_free_position(12));
        assert!(is_free_position(13));
        assert!(!is_free_position(14));
        assert!(is_free_position(26));
        assert_eq!(card_index_for(1), 1);
        assert_eq!(card_index_for(13), 1);
        assert_eq!(card_index_for(14), 2);
    }

    #[test]
    fn cards_hold_twelve_stamps_and_a_reward() {
        let drinks: Vec<PaidDrink> = (0..14).map(|i| drink(1_000 + i, 50)).collect();
        let cards = build_cards(&drinks);

        assert_eq!(cards.len(), 2);
        assert_eq!(cards[0].stamps.len(), STAMPS_PER_CARD as usize);
        assert!(cards[0].complete);
        let reward = cards[0].reward.as_ref().expect("free drink on full card");
        assert_eq!(reward.position, 13);
        assert_eq!(cards[1].stamps.len(), 1);
        assert!(!cards[1].complete);
        assert!(cards[1].reward.is_none());
    }

    #[test]
    fn empty_month_still_renders_one_card() {
        let cards = build_cards(&[]);
        assert_eq!(cards.len(), 1);
        assert!(cards[0].stamps.is_empty());
    }

    #[test]
    fn discount_skips_free_positions_within_the_open_window() {
        let tz = resolve_tz("Europe/Berlin");
        let drinks: Vec<PaidDrink> = (0..13)
            .map(|i| drink(ts(&tz, 2026, 9, 1, 8) + i * 3_600, 50))
            .collect();

        assert_eq!(discounted_amount_cents(&drinks, 0, &tz), 12 * 50);
    }

    #[test]
    fn payment_cutoff_does_not_shift_stamp_positions() {
        let tz = resolve_tz("Europe/Berlin");
        let drinks: Vec<PaidDrink> = (0..13)
            .map(|i| drink(ts(&tz, 2026, 9, 1, 8) + i * 3_600, 50))
            .collect();
        // Paid off after the first eight drinks; the 13th stays free regardless.
        let cutoff = drinks[8].consumed_at;

        assert_eq!(discounted_amount_cents(&drinks, cutoff, &tz), 4 * 50);
    }

    #[test]
    fn month_rollover_resets_the_card() {
        let tz = resolve_tz("Europe/Berlin");
        let mut drinks: Vec<PaidDrink> = (0..12)
            .map(|i| drink(ts(&tz, 2026, 9, 20, 8) + i * 3_600, 50))
            .collect();
        // First drink of the next month: a fresh card, so it is not free.
        drinks.push(drink(ts(&tz, 2026, 10, 1, 8), 50));

        assert_eq!(discounted_amount_cents(&drinks, 0, &tz), 13 * 50);
    }

    #[test]
    fn month_bounds_follow_the_configured_timezone() {
        let tz = resolve_tz("Europe/Berlin");
        let (start, end) = month_bounds(&tz, 2026, 9).unwrap();

        assert_eq!(start, ts(&tz, 2026, 9, 1, 0));
        assert_eq!(end, ts(&tz, 2026, 10, 1, 0));
        // 00:30 local on the 1st belongs to the new month, not the previous one.
        assert_eq!(local_month(start + 1_800, &tz), (2026, 9));
        assert_eq!(local_month(start - 1_800, &tz), (2026, 8));
    }

    #[test]
    fn december_rolls_over_into_january() {
        let tz = resolve_tz("UTC");
        let (start, end) = month_bounds(&tz, 2026, 12).unwrap();
        assert_eq!(start, ts(&tz, 2026, 12, 1, 0));
        assert_eq!(end, ts(&tz, 2027, 1, 1, 0));
    }

    #[test]
    fn month_parsing_rejects_garbage() {
        assert_eq!(parse_month("2026-09").unwrap(), (2026, 9));
        assert!(parse_month("2026").is_err());
        assert!(parse_month("2026-13").is_err());
        assert!(parse_month("nope-xx").is_err());
    }
}
