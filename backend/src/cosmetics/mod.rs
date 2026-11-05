//! Lootboxes and unlockable cosmetics.
//!
//! Boxes are earned (never bought): one per unlocked achievement, per shopping
//! trip (positive expense entry), per filled Stempelkarte and for the first
//! machine cleaning of each week. Every earned box is a row in `lootbox_grants`
//! keyed by its source, so syncing is idempotent and retroactive.
//! Opening a box rolls a random cosmetic; duplicates turn into beans, which can
//! be spent to craft a specific item. Cosmetics are purely visual.

pub mod catalog;
pub mod rewards;

use std::collections::{HashMap, HashSet};

use axum::{
    Json, Router,
    extract::{Path, State},
    http::header,
    response::{IntoResponse, Response},
    routing::{get, patch, post},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    AppState,
    achievements::{AchievementEvent, evaluate_global, reward_epoch},
    auth::CurrentUser,
    db::now_ts,
    error::{ApiError, ApiResult},
    loyalty,
    notifications::notify_unlocks,
    stats::resolve_tz,
};
use catalog::{CATALOG, CosmeticDef, Rarity, Slot, TOTAL_WEIGHT};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/cosmetics/me", get(my_locker))
        .route("/cosmetics/equipped", patch(equip_cosmetic))
        .route("/cosmetics/craft", post(craft_cosmetic))
        .route("/lootboxes/open", post(open_lootbox))
        .route("/cosmetics/artwork/{item_id}", get(private_artwork))
}

pub(crate) fn is_member(user: &CurrentUser) -> bool {
    user.user_type == "member"
}

async fn is_member_id(state: &AppState, user_id: &str) -> ApiResult<bool> {
    let row: Option<(String,)> = sqlx::query_as("SELECT user_type FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_optional(&state.db)
        .await?;
    Ok(matches!(row, Some((user_type,)) if user_type == "member"))
}

/// Private avatar artwork (team photos, third-party stickers), members only.
#[utoipa::path(
    get,
    path = "/api/v1/cosmetics/artwork/{item_id}",
    params(("item_id" = String, Path)),
    responses(
        (status = 200, description = "WebP image", content_type = "image/webp"),
        (status = 403, description = "Guests may not see private artwork"),
        (status = 404, description = "Unknown item or missing file")
    )
)]
pub(crate) async fn private_artwork(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(item_id): Path<String>,
) -> ApiResult<Response> {
    if !is_member(&user) {
        return Err(ApiError::Forbidden);
    }
    // The file name comes from the static catalog, never from the request.
    let item = catalog::find(&item_id)
        .filter(|item| item.private_artwork)
        .ok_or(ApiError::NotFound)?;
    let path =
        std::path::Path::new(&state.config.cosmetics_private_dir).join(format!("{}.webp", item.id));
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|_| ApiError::NotFound)?;
    Ok((
        [
            (header::CONTENT_TYPE, "image/webp"),
            (header::CACHE_CONTROL, "private, max-age=86400"),
        ],
        bytes,
    )
        .into_response())
}

#[derive(Serialize, ToSchema)]
pub struct CosmeticItem {
    pub id: String,
    pub slot: Slot,
    pub rarity: Rarity,
    pub name: String,
    pub description: String,
    /// Emoji fallback for items without dedicated artwork.
    pub glyph: Option<String>,
    /// Image path relative to the frontend base URL (avatars only).
    pub artwork_path: Option<String>,
    pub craftable: bool,
    /// Beans needed to craft the item; `None` when it can only drop from a box.
    pub craft_cost: Option<i64>,
    pub owned: bool,
    pub equipped: bool,
    pub acquired_at: Option<i64>,
}

#[derive(Serialize, ToSchema, Default)]
pub struct EquippedCosmetics {
    pub frame: Option<String>,
    pub avatar: Option<String>,
    pub cup: Option<String>,
    pub stamp: Option<String>,
    pub title: Option<String>,
    pub skin: Option<String>,
}

#[derive(Serialize, ToSchema)]
pub struct RarityInfo {
    pub rarity: Rarity,
    pub label: String,
    pub drop_chance_percent: f64,
    pub duplicate_beans: i64,
    pub craft_cost: i64,
}

#[derive(Serialize, ToSchema, Default)]
pub struct LootboxSources {
    pub achievement: i64,
    pub shopping: i64,
    pub stamp_card: i64,
    /// Weekly chores (cleaning, grounds, water, descaling).
    pub cleaning: i64,
    pub daily: i64,
    pub streak: i64,
    pub advent: i64,
    pub milestone: i64,
    pub payment: i64,
    /// Lucky wheel prizes.
    pub wheel: i64,
}

#[derive(Serialize, ToSchema)]
pub struct LockerResponse {
    pub boxes_available: i64,
    pub boxes_earned: i64,
    pub boxes_opened: i64,
    pub beans: i64,
    pub sources: LootboxSources,
    pub equipped: EquippedCosmetics,
    pub items: Vec<CosmeticItem>,
    pub rarities: Vec<RarityInfo>,
}

#[derive(Serialize, ToSchema)]
pub struct LootboxOpenResponse {
    pub item: CosmeticItem,
    pub duplicate: bool,
    pub beans_awarded: i64,
    pub unlocked_achievements: Vec<AchievementEvent>,
    pub locker: LockerResponse,
}

#[derive(Deserialize, ToSchema)]
pub struct CraftRequest {
    pub item_id: String,
}

#[derive(Serialize, ToSchema)]
pub struct CraftResponse {
    pub item: CosmeticItem,
    pub unlocked_achievements: Vec<AchievementEvent>,
    pub locker: LockerResponse,
}

#[derive(Deserialize, ToSchema)]
pub struct EquipRequest {
    pub slot: Slot,
    /// Item to equip in `slot`; `null` resets the slot to the default look.
    pub item_id: Option<String>,
}

/// Cosmetics other people can see (leaderboard, coffee twins).
#[derive(Serialize, ToSchema, Clone, Default, Debug, PartialEq)]
pub struct PublicCosmetics {
    pub avatar: Option<String>,
    /// Image path of the equipped avatar, relative to the frontend base URL.
    pub avatar_artwork: Option<String>,
    pub frame: Option<String>,
    /// Display text of the equipped title.
    pub title: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/v1/cosmetics/me",
    responses((status = 200, body = LockerResponse))
)]
pub(crate) async fn my_locker(
    State(state): State<AppState>,
    user: CurrentUser,
) -> ApiResult<Json<LockerResponse>> {
    sync_grants(&state, &user.id).await?;
    Ok(Json(load_locker(&state, &user.id).await?))
}

#[utoipa::path(
    post,
    path = "/api/v1/lootboxes/open",
    responses(
        (status = 200, body = LootboxOpenResponse),
        (status = 409, description = "No unopened lootboxes left")
    )
)]
pub(crate) async fn open_lootbox(
    State(state): State<AppState>,
    user: CurrentUser,
) -> ApiResult<Json<LootboxOpenResponse>> {
    sync_grants(&state, &user.id).await?;
    let now = now_ts();
    let item = catalog::roll_item(
        catalog::random_u64(),
        catalog::random_u64(),
        is_member(&user),
    );
    let opening_id = Uuid::new_v4().to_string();

    let mut tx = state.db.begin().await?;
    // One statement: the "has a box left" check and the insert cannot interleave
    // with a concurrent opening.
    let inserted = sqlx::query(
        "INSERT INTO lootbox_openings
         (id, user_id, item_id, rarity, duplicate, beans_awarded, opened_at)
         SELECT ?, ?, ?, ?, owned.value, CASE WHEN owned.value THEN ? ELSE 0 END, unixepoch()
         FROM (SELECT EXISTS(
                 SELECT 1 FROM user_cosmetics WHERE user_id = ? AND item_id = ?
               ) AS value) AS owned
         WHERE (SELECT COUNT(*) FROM lootbox_grants WHERE user_id = ?)
             > (SELECT COUNT(*) FROM lootbox_openings WHERE user_id = ?)",
    )
    .bind(&opening_id)
    .bind(&user.id)
    .bind(item.id)
    .bind(item.rarity.as_str())
    .bind(item.rarity.duplicate_beans())
    .bind(&user.id)
    .bind(item.id)
    .bind(&user.id)
    .bind(&user.id)
    .execute(&mut *tx)
    .await?;
    if inserted.rows_affected() == 0 {
        return Err(ApiError::Conflict("no unopened lootboxes left".into()));
    }
    sqlx::query(
        "INSERT OR IGNORE INTO user_cosmetics (user_id, item_id, source, acquired_at)
         VALUES (?, ?, 'lootbox', ?)",
    )
    .bind(&user.id)
    .bind(item.id)
    .bind(now)
    .execute(&mut *tx)
    .await?;
    let (duplicate, beans_awarded): (bool, i64) =
        sqlx::query_as("SELECT duplicate, beans_awarded FROM lootbox_openings WHERE id = ?")
            .bind(&opening_id)
            .fetch_one(&mut *tx)
            .await?;
    tx.commit().await?;

    let unlocked_achievements = after_locker_change(&state, &user, now).await?;
    let locker = load_locker(&state, &user.id).await?;
    Ok(Json(LootboxOpenResponse {
        item: locker_item(&locker, item.id)?,
        duplicate,
        beans_awarded,
        unlocked_achievements,
        locker,
    }))
}

#[utoipa::path(
    post,
    path = "/api/v1/cosmetics/craft",
    request_body = CraftRequest,
    responses(
        (status = 200, body = CraftResponse),
        (status = 409, description = "Already owned or not enough beans")
    )
)]
pub(crate) async fn craft_cosmetic(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(input): Json<CraftRequest>,
) -> ApiResult<Json<CraftResponse>> {
    let item = catalog::find(&input.item_id)
        .filter(|item| item.visible_to(is_member(&user)))
        .ok_or(ApiError::NotFound)?;
    if !item.craftable {
        return Err(ApiError::BadRequest(
            "this item can only be found in lootboxes".into(),
        ));
    }
    let cost = item.rarity.craft_cost();
    let now = now_ts();

    let mut tx = state.db.begin().await?;
    let inserted = sqlx::query(
        "INSERT INTO cosmetic_crafts (id, user_id, item_id, beans_spent, crafted_at)
         SELECT ?, ?, ?, ?, ?
         WHERE NOT EXISTS (SELECT 1 FROM user_cosmetics WHERE user_id = ? AND item_id = ?)
           AND COALESCE((SELECT SUM(beans_awarded) FROM lootbox_openings WHERE user_id = ?), 0)
             - COALESCE((SELECT SUM(beans_spent) FROM cosmetic_crafts WHERE user_id = ?), 0) >= ?",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(&user.id)
    .bind(item.id)
    .bind(cost)
    .bind(now)
    .bind(&user.id)
    .bind(item.id)
    .bind(&user.id)
    .bind(&user.id)
    .bind(cost)
    .execute(&mut *tx)
    .await?;
    if inserted.rows_affected() == 0 {
        let owned = owns(&mut *tx, &user.id, item.id).await?;
        return Err(ApiError::Conflict(if owned {
            "you already own this item".into()
        } else {
            "not enough beans".into()
        }));
    }
    sqlx::query(
        "INSERT INTO user_cosmetics (user_id, item_id, source, acquired_at)
         VALUES (?, ?, 'craft', ?)",
    )
    .bind(&user.id)
    .bind(item.id)
    .bind(now)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    let unlocked_achievements = after_locker_change(&state, &user, now).await?;
    let locker = load_locker(&state, &user.id).await?;
    Ok(Json(CraftResponse {
        item: locker_item(&locker, item.id)?,
        unlocked_achievements,
        locker,
    }))
}

#[utoipa::path(
    patch,
    path = "/api/v1/cosmetics/equipped",
    request_body = EquipRequest,
    responses((status = 200, body = LockerResponse))
)]
pub(crate) async fn equip_cosmetic(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(input): Json<EquipRequest>,
) -> ApiResult<Json<LockerResponse>> {
    if !input.slot.equippable() {
        return Err(ApiError::BadRequest(
            "collectibles cannot be equipped".into(),
        ));
    }
    match input.item_id.as_deref() {
        None => {
            sqlx::query("DELETE FROM user_equipped_cosmetics WHERE user_id = ? AND slot = ?")
                .bind(&user.id)
                .bind(input.slot.as_str())
                .execute(&state.db)
                .await?;
        }
        Some(item_id) => {
            let item = catalog::find(item_id)
                .filter(|item| item.visible_to(is_member(&user)))
                .ok_or(ApiError::NotFound)?;
            if item.slot != input.slot {
                return Err(ApiError::BadRequest("item does not fit this slot".into()));
            }
            if !owns(&state.db, &user.id, item.id).await? {
                return Err(ApiError::Forbidden);
            }
            sqlx::query(
                "INSERT INTO user_equipped_cosmetics (user_id, slot, item_id)
                 VALUES (?, ?, ?)
                 ON CONFLICT(user_id, slot) DO UPDATE SET item_id = excluded.item_id",
            )
            .bind(&user.id)
            .bind(input.slot.as_str())
            .bind(item.id)
            .execute(&state.db)
            .await?;
        }
    }
    Ok(Json(load_locker(&state, &user.id).await?))
}

/// When the user's locker was first synced, onboarding them on the first call.
///
/// Onboarding unlocks every achievement the user already qualifies for (each
/// earns a crate like any other). Shopping trips and full stamp cards only earn
/// crates from this moment on, so past supply runs are not paid out in bulk.
pub async fn ensure_onboarded(state: &AppState, user_id: &str) -> ApiResult<i64> {
    let existing: Option<(i64,)> =
        sqlx::query_as("SELECT onboarded_at FROM lootbox_profiles WHERE user_id = ?")
            .bind(user_id)
            .fetch_optional(&state.db)
            .await?;
    if let Some((onboarded_at,)) = existing {
        return Ok(onboarded_at);
    }

    let now = now_ts();
    // Catch up on achievements earned from history (full cards, shopping, ...).
    evaluate_global(state, user_id, now).await?;
    sqlx::query("INSERT OR IGNORE INTO lootbox_profiles (user_id, onboarded_at) VALUES (?, ?)")
        .bind(user_id)
        .bind(now)
        .execute(&state.db)
        .await?;
    let (onboarded_at,): (i64,) =
        sqlx::query_as("SELECT onboarded_at FROM lootbox_profiles WHERE user_id = ?")
            .bind(user_id)
            .fetch_one(&state.db)
            .await?;
    Ok(onboarded_at)
}

/// Records every lootbox the user is entitled to but has no grant row for yet.
/// Idempotent; safe to call on every read. Every unlocked achievement counts
/// (including ones from before lootboxes existed); shopping trips and full
/// stamp cards only count from onboarding on.
///
/// Every write is skipped once a drink of the user is undone mid-sync (the
/// reward epoch moved on), because it was computed from data that may still
/// contain that drink; the undo re-syncs afterwards.
pub async fn sync_grants(state: &AppState, user_id: &str) -> ApiResult<Vec<String>> {
    let onboarded_at = ensure_onboarded(state, user_id).await?;
    let epoch = reward_epoch(state, user_id).await?;
    let mut new_grants: Vec<String> = Vec::new();
    let mut record = |source: &str, count: u64| {
        new_grants.extend(std::iter::repeat_n(source.to_string(), count as usize));
    };

    let result = sqlx::query(
        "INSERT OR IGNORE INTO lootbox_grants (id, user_id, source, source_ref, granted_at)
         SELECT lower(hex(randomblob(16))), au.user_id, 'achievement', au.achievement_id, au.unlocked_at
         FROM achievement_unlocks au
         WHERE au.user_id = ?
           AND (SELECT reward_epoch FROM users WHERE id = ?) = ?",
    )
    .bind(user_id)
    .bind(user_id)
    .bind(epoch)
    .execute(&state.db)
    .await?;
    record("achievement", result.rows_affected());
    // At most one shopping crate per person and (UTC) day, for purchases of at
    // least 1 EUR, so +1 ct / -1 ct entry pairs can't farm crates.
    let result = sqlx::query(
        "INSERT OR IGNORE INTO lootbox_grants (id, user_id, source, source_ref, granted_at)
         SELECT lower(hex(randomblob(16))), user_id, 'shopping',
                date(logged_at, 'unixepoch'), MIN(logged_at)
         FROM expense_entries
         WHERE user_id = ? AND amount_cents >= 100 AND logged_at >= ?
           AND (SELECT reward_epoch FROM users WHERE id = ?) = ?
         GROUP BY user_id, date(logged_at, 'unixepoch')",
    )
    .bind(user_id)
    .bind(onboarded_at)
    .bind(user_id)
    .bind(epoch)
    .execute(&state.db)
    .await?;
    record("shopping", result.rows_affected());

    let existing: HashSet<(String, String)> = sqlx::query_as::<_, (String, String)>(
        "SELECT source, source_ref FROM lootbox_grants
         WHERE user_id = ?
           AND source IN ('stamp_card', 'daily', 'streak', 'advent', 'milestone', 'payment')",
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .collect();
    let mut missing: Vec<(&'static str, String, i64)> = Vec::new();

    for month in loyalty::card_progress_by_month(state, user_id).await? {
        for (index, completed_at) in month.card_completed_at.iter().enumerate() {
            if *completed_at >= onboarded_at {
                missing.push((
                    "stamp_card",
                    stamp_card_ref(&month.month, index as i64 + 1),
                    *completed_at,
                ));
            }
        }
    }

    let tz = resolve_tz(&state.config.timezone);
    // Paying your tab keeps the coffee fund alive: one crate per local month
    // with a confirmed payment >= 1 EUR, so splitting a tab into many small
    // payments cannot farm crates.
    let payments: Vec<(i64,)> = sqlx::query_as(
        "SELECT paid_at FROM payments
         WHERE user_id = ? AND status = 'confirmed' AND amount_cents >= 100 AND paid_at >= ?
         ORDER BY paid_at ASC",
    )
    .bind(user_id)
    .bind(onboarded_at)
    .fetch_all(&state.db)
    .await?;
    for (paid_at,) in payments {
        // The first payment of each month is the one that counts.
        missing.push(("payment", payment_month_ref(paid_at, &tz), paid_at));
    }

    let consumed_at: Vec<i64> = sqlx::query_as::<_, (i64,)>(
        "SELECT consumed_at FROM consumptions WHERE user_id = ? ORDER BY consumed_at ASC",
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .map(|(timestamp,)| timestamp)
    .collect();
    let daily_granted: Vec<chrono::NaiveDate> = existing
        .iter()
        .filter(|(source, _)| source == "daily")
        .filter_map(|(_, day)| day.parse().ok())
        .collect();
    let activity = rewards::activity_rewards(&consumed_at, &tz, onboarded_at, &daily_granted);
    let now = now_ts();
    for day in &activity.daily {
        missing.push(("daily", day.to_string(), now));
    }
    for day in &activity.streak {
        missing.push(("streak", day.to_string(), now));
    }
    for day in &activity.advent {
        missing.push(("advent", day.to_string(), now));
    }
    for (count, timestamp) in &activity.milestones {
        missing.push(("milestone", count.to_string(), *timestamp));
    }

    for (source, source_ref, granted_at) in missing {
        if existing.contains(&(source.to_string(), source_ref.clone())) {
            continue;
        }
        if grant_at_epoch(state, user_id, source, &source_ref, granted_at, Some(epoch)).await? {
            new_grants.push(source.to_string());
        }
    }
    Ok(new_grants)
}

/// `YYYY-MM` of the local month a payment falls in.
pub fn payment_month_ref(paid_at: i64, tz: &chrono_tz::Tz) -> String {
    let (year, month) = loyalty::local_month(paid_at, tz);
    format!("{year:04}-{month:02}")
}

pub fn stamp_card_ref(month: &str, card_index: i64) -> String {
    format!("{month}#{card_index}")
}

pub async fn grant(
    state: &AppState,
    user_id: &str,
    source: &str,
    source_ref: &str,
    granted_at: i64,
) -> ApiResult<bool> {
    grant_at_epoch(state, user_id, source, source_ref, granted_at, None).await
}

/// Like [`grant`], but only while the user's reward epoch still is `epoch`.
async fn grant_at_epoch(
    state: &AppState,
    user_id: &str,
    source: &str,
    source_ref: &str,
    granted_at: i64,
    epoch: Option<i64>,
) -> ApiResult<bool> {
    let inserted = sqlx::query(
        "INSERT OR IGNORE INTO lootbox_grants (id, user_id, source, source_ref, granted_at)
         SELECT ?, ?, ?, ?, ?
         WHERE ? IS NULL OR (SELECT reward_epoch FROM users WHERE id = ?) = ?",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(user_id)
    .bind(source)
    .bind(source_ref)
    .bind(granted_at)
    .bind(epoch)
    .bind(user_id)
    .bind(epoch)
    .execute(&state.db)
    .await?;
    Ok(inserted.rows_affected() > 0)
}

/// Undo support: removes the crates since `since` (the undone drink's time)
/// that drinks may have earned, plus the crates of achievements that are no
/// longer unlocked. Run it after [`crate::achievements::revoke_unlocks_since`];
/// the caller re-syncs afterwards, restoring whatever is still earned.
pub(crate) async fn revoke_grants_since(
    conn: &mut sqlx::SqliteConnection,
    user_id: &str,
    since: i64,
) -> ApiResult<()> {
    sqlx::query(
        "DELETE FROM lootbox_grants
         WHERE user_id = ? AND granted_at >= ?
           AND (
               source IN ('stamp_card', 'daily', 'streak', 'advent', 'milestone')
               OR (source = 'achievement' AND source_ref NOT IN (
                   SELECT achievement_id FROM achievement_unlocks WHERE user_id = ?
               ))
           )",
    )
    .bind(user_id)
    .bind(since)
    .bind(user_id)
    .execute(conn)
    .await?;
    Ok(())
}

/// Re-evaluates achievements that depend on the locker (and the grants they earn).
async fn after_locker_change(
    state: &AppState,
    user: &CurrentUser,
    now: i64,
) -> ApiResult<Vec<AchievementEvent>> {
    let unlocks = evaluate_global(state, &user.id, now).await?;
    if !unlocks.is_empty() {
        sync_grants(state, &user.id).await?;
        notify_unlocks(state, user, &unlocks).await;
    }
    Ok(unlocks)
}

async fn owns<'e, E>(executor: E, user_id: &str, item_id: &str) -> ApiResult<bool>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    let row: Option<(i64,)> =
        sqlx::query_as("SELECT 1 FROM user_cosmetics WHERE user_id = ? AND item_id = ?")
            .bind(user_id)
            .bind(item_id)
            .fetch_optional(executor)
            .await?;
    Ok(row.is_some())
}

fn locker_item(locker: &LockerResponse, item_id: &str) -> ApiResult<CosmeticItem> {
    locker
        .items
        .iter()
        .find(|item| item.id == item_id)
        .map(|item| CosmeticItem {
            id: item.id.clone(),
            slot: item.slot,
            rarity: item.rarity,
            name: item.name.clone(),
            description: item.description.clone(),
            glyph: item.glyph.clone(),
            artwork_path: item.artwork_path.clone(),
            craftable: item.craftable,
            craft_cost: item.craft_cost,
            owned: item.owned,
            equipped: item.equipped,
            acquired_at: item.acquired_at,
        })
        .ok_or(ApiError::NotFound)
}

pub async fn load_locker(state: &AppState, user_id: &str) -> ApiResult<LockerResponse> {
    let mut sources = LootboxSources::default();
    let source_counts: Vec<(String, i64)> = sqlx::query_as(
        "SELECT source, COUNT(*) FROM lootbox_grants WHERE user_id = ? GROUP BY source",
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;
    for (source, count) in source_counts {
        match source.as_str() {
            "achievement" => sources.achievement = count,
            "shopping" => sources.shopping = count,
            "stamp_card" => sources.stamp_card = count,
            "cleaning" => sources.cleaning = count,
            "daily" => sources.daily = count,
            "streak" => sources.streak = count,
            "advent" => sources.advent = count,
            "milestone" => sources.milestone = count,
            "payment" => sources.payment = count,
            "wheel" => sources.wheel = count,
            _ => {}
        }
    }
    let boxes_earned = sources.achievement
        + sources.shopping
        + sources.stamp_card
        + sources.cleaning
        + sources.daily
        + sources.streak
        + sources.advent
        + sources.milestone
        + sources.payment
        + sources.wheel;

    let (boxes_opened,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM lootbox_openings WHERE user_id = ?")
            .bind(user_id)
            .fetch_one(&state.db)
            .await?;
    let (beans,): (i64,) = sqlx::query_as(
        "SELECT COALESCE((SELECT SUM(beans_awarded) FROM lootbox_openings WHERE user_id = ?), 0)
              - COALESCE((SELECT SUM(beans_spent) FROM cosmetic_crafts WHERE user_id = ?), 0)",
    )
    .bind(user_id)
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;

    let owned: HashMap<String, i64> = sqlx::query_as::<_, (String, i64)>(
        "SELECT item_id, acquired_at FROM user_cosmetics WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?
    .into_iter()
    .collect();

    let equipped_rows: Vec<(String, String)> =
        sqlx::query_as("SELECT slot, item_id FROM user_equipped_cosmetics WHERE user_id = ?")
            .bind(user_id)
            .fetch_all(&state.db)
            .await?;
    let mut equipped = EquippedCosmetics::default();
    let mut equipped_ids = HashSet::new();
    for (slot, item_id) in equipped_rows {
        let Some(item) = catalog::find(&item_id).filter(|item| item.slot.as_str() == slot) else {
            continue;
        };
        equipped_ids.insert(item.id);
        let target = match item.slot {
            Slot::Frame => &mut equipped.frame,
            Slot::Avatar => &mut equipped.avatar,
            Slot::Cup => &mut equipped.cup,
            Slot::Stamp => &mut equipped.stamp,
            Slot::Title => &mut equipped.title,
            Slot::Skin => &mut equipped.skin,
            Slot::Collectible => continue,
        };
        *target = Some(item.id.to_string());
    }

    let member = is_member_id(state, user_id).await?;
    let items = CATALOG
        .iter()
        .filter(|item| item.visible_to(member))
        .map(|item| {
            to_item(
                item,
                owned.get(item.id).copied(),
                equipped_ids.contains(item.id),
            )
        })
        .collect();

    Ok(LockerResponse {
        boxes_available: (boxes_earned - boxes_opened).max(0),
        boxes_earned,
        boxes_opened,
        beans,
        sources,
        equipped,
        items,
        rarities: rarity_table(),
    })
}

fn to_item(item: &CosmeticDef, acquired_at: Option<i64>, equipped: bool) -> CosmeticItem {
    CosmeticItem {
        id: item.id.into(),
        slot: item.slot,
        rarity: item.rarity,
        name: item.name.into(),
        description: item.description.into(),
        glyph: item.glyph.map(Into::into),
        artwork_path: item.artwork_path(),
        craftable: item.craftable,
        craft_cost: item.craftable.then(|| item.rarity.craft_cost()),
        owned: acquired_at.is_some(),
        equipped,
        acquired_at,
    }
}

fn rarity_table() -> Vec<RarityInfo> {
    Rarity::ALL
        .iter()
        .map(|rarity| RarityInfo {
            rarity: *rarity,
            label: rarity.label().into(),
            drop_chance_percent: rarity.weight() as f64 * 100.0 / TOTAL_WEIGHT as f64,
            duplicate_beans: rarity.duplicate_beans(),
            craft_cost: rarity.craft_cost(),
        })
        .collect()
}

/// Equipped avatar, frame and title for every user that has any of them.
pub async fn public_cosmetics_by_user(
    state: &AppState,
) -> ApiResult<HashMap<String, PublicCosmetics>> {
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT user_id, slot, item_id
         FROM user_equipped_cosmetics
         WHERE slot IN ('avatar', 'frame', 'title')",
    )
    .fetch_all(&state.db)
    .await?;
    let mut by_user: HashMap<String, PublicCosmetics> = HashMap::new();
    for (user_id, slot, item_id) in rows {
        let Some(item) = catalog::find(&item_id).filter(|item| item.slot.as_str() == slot) else {
            continue;
        };
        let entry = by_user.entry(user_id).or_default();
        match item.slot {
            Slot::Avatar => {
                entry.avatar = Some(item.id.into());
                entry.avatar_artwork = item.artwork_path();
            }
            Slot::Frame => entry.frame = Some(item.id.into()),
            Slot::Title => entry.title = Some(item.name.into()),
            _ => {}
        }
    }
    Ok(by_user)
}

#[cfg(test)]
pub(crate) mod test_support {
    use uuid::Uuid;

    use crate::{
        AppState,
        auth::CurrentUser,
        config::Config,
        db::{connect, seed_defaults},
    };

    pub fn member(id: &str) -> CurrentUser {
        CurrentUser {
            id: id.into(),
            user_type: "member".into(),
            gitlab_id: None,
            display_name: id.to_uppercase(),
            leaderboard_opt_in: true,
            achievement_sharing_opt_out: false,
            mattermost_opt_in: false,
            stock_manager: true,
            paypal_link: None,
        }
    }

    pub async fn state_with_users(user_ids: &[&str]) -> AppState {
        let db_path = format!("/tmp/coffee-cosmetics-{}.db", Uuid::new_v4());
        let db = connect(&format!("sqlite://{db_path}")).await.unwrap();
        for id in user_ids {
            sqlx::query(
                "INSERT INTO users
                 (id, user_type, display_name, leaderboard_opt_in,
                  achievement_sharing_opt_out, mattermost_opt_in, created_at, updated_at)
                 VALUES (?, 'member', ?, 1, 0, 0, 1, 1)",
            )
            .bind(id)
            .bind(id.to_uppercase())
            .execute(&db)
            .await
            .unwrap();
        }
        // Machines, beverages and achievement definitions.
        seed_defaults(&db).await.unwrap();
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
                auto_payment_reminder: None,
                timezone: "Europe/Berlin".into(),
                session_ttl: std::time::Duration::from_secs(60),
                cosmetics_private_dir: "../cosmetics_private".into(),
            },
            db,
            http: reqwest::Client::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use axum::extract::State;
    use chrono::TimeZone;

    use super::test_support::{member, state_with_users};
    use super::*;

    async fn unlock_achievement(state: &AppState, user_id: &str, achievement_id: &str) {
        unlock_achievement_at(state, user_id, achievement_id, 1).await;
    }

    async fn unlock_achievement_at(state: &AppState, user_id: &str, achievement_id: &str, at: i64) {
        sqlx::query(
            "INSERT INTO achievement_unlocks (id, user_id, achievement_id, unlocked_at)
             VALUES (?, ?, ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(user_id)
        .bind(achievement_id)
        .bind(at)
        .execute(&state.db)
        .await
        .unwrap();
    }

    async fn add_expense(state: &AppState, user_id: &str, id: &str, amount_cents: i64, at: i64) {
        sqlx::query(
            "INSERT INTO expense_entries
             (id, user_id, created_by_user_id, description, amount_cents, logged_at, created_at)
             VALUES (?, ?, ?, 'Supplies', ?, ?, ?)",
        )
        .bind(id)
        .bind(user_id)
        .bind(user_id)
        .bind(amount_cents)
        .bind(at)
        .bind(at)
        .execute(&state.db)
        .await
        .unwrap();
    }

    async fn add_coffees(state: &AppState, user_id: &str, count: i64, start: i64) {
        for i in 0..count {
            sqlx::query(
                "INSERT INTO consumptions (id, user_id, machine_id, beverage_id, consumed_at, created_at)
                 VALUES (?, ?, 'main', 'coffee', ?, ?)",
            )
            .bind(Uuid::new_v4().to_string())
            .bind(user_id)
            .bind(start + i * 60)
            .bind(start + i * 60)
            .execute(&state.db)
            .await
            .unwrap();
        }
    }

    async fn give_item(state: &AppState, user_id: &str, item_id: &str) {
        sqlx::query(
            "INSERT INTO user_cosmetics (user_id, item_id, source, acquired_at)
             VALUES (?, ?, 'lootbox', 1)",
        )
        .bind(user_id)
        .bind(item_id)
        .execute(&state.db)
        .await
        .unwrap();
    }

    async fn give_beans(state: &AppState, user_id: &str, beans: i64) {
        sqlx::query(
            "INSERT INTO lootbox_openings
             (id, user_id, item_id, rarity, duplicate, beans_awarded, opened_at)
             VALUES (?, ?, 'frame_crema', 'common', 1, ?, 1)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(user_id)
        .bind(beans)
        .execute(&state.db)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn every_achievement_earns_a_crate_but_old_shopping_and_cards_do_not() {
        let state = state_with_users(&["alice"]).await;
        // A long-time user: many achievements, shopping trips and two full cards.
        for achievement in ["first_coffee", "busy_day_10", "double_century", "full_menu"] {
            unlock_achievement(&state, "alice", achievement).await;
        }
        add_expense(&state, "alice", "beans", 1999, 1).await;
        add_expense(&state, "alice", "milk", 450, 2).await;
        let october = chrono_tz::Europe::Berlin
            .with_ymd_and_hms(2026, 10, 1, 9, 0, 0)
            .unwrap()
            .timestamp();
        add_coffees(&state, "alice", 25, october).await;

        sync_grants(&state, "alice").await.unwrap();
        sync_grants(&state, "alice").await.unwrap();
        let locker = load_locker(&state, "alice").await.unwrap();

        // Onboarding caught up on achievements from history (full card, free
        // drink, shopping) on top of the four existing ones ...
        let (unlocked,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM achievement_unlocks WHERE user_id = 'alice'")
                .fetch_one(&state.db)
                .await
                .unwrap();
        assert_eq!(unlocked, 7);
        // ... and every one of them is worth a crate, exactly once.
        assert_eq!(locker.sources.achievement, unlocked);
        // Past shopping trips and full cards are not paid out.
        assert_eq!(locker.sources.shopping, 0);
        assert_eq!(locker.sources.stamp_card, 0);
        assert_eq!(locker.boxes_available, unlocked);
        assert_eq!(locker.items.len(), CATALOG.len());
    }

    #[tokio::test]
    async fn shopping_earns_at_most_one_crate_per_day_and_ignores_tiny_entries() {
        let state = state_with_users(&["alice"]).await;
        let onboarded_at = ensure_onboarded(&state, "alice").await.unwrap();
        let day = 86_400 * (onboarded_at / 86_400 + 1) + 3_600; // 01:00 UTC tomorrow
        // Farming attempt: tiny +/- pairs.
        for i in 0..5 {
            add_expense(&state, "alice", &format!("plus-{i}"), 1, day + i).await;
            add_expense(&state, "alice", &format!("minus-{i}"), -1, day + i).await;
        }
        assert_eq!(
            load_locker_after_sync(&state, "alice")
                .await
                .sources
                .shopping,
            0
        );
        // Two real trips on the same day count once; a second day counts again.
        add_expense(&state, "alice", "beans", 1999, day + 60).await;
        add_expense(&state, "alice", "milk", 450, day + 120).await;
        assert_eq!(
            load_locker_after_sync(&state, "alice")
                .await
                .sources
                .shopping,
            1
        );
        add_expense(&state, "alice", "sugar", 300, day + 86_400).await;
        assert_eq!(
            load_locker_after_sync(&state, "alice")
                .await
                .sources
                .shopping,
            2
        );
    }

    async fn load_locker_after_sync(state: &AppState, user_id: &str) -> LockerResponse {
        sync_grants(state, user_id).await.unwrap();
        load_locker(state, user_id).await.unwrap()
    }

    #[tokio::test]
    async fn guests_never_see_private_artwork() {
        let state = state_with_users(&["alice", "guest"]).await;
        sqlx::query("UPDATE users SET user_type = 'guest' WHERE id = 'guest'")
            .execute(&state.db)
            .await
            .unwrap();
        let mut guest = member("guest");
        guest.user_type = "guest".into();

        let guest_locker = load_locker(&state, "guest").await.unwrap();
        assert!(
            guest_locker
                .items
                .iter()
                .all(|item| { !catalog::find(&item.id).unwrap().private_artwork })
        );
        let member_locker = load_locker(&state, "alice").await.unwrap();
        assert_eq!(member_locker.items.len(), CATALOG.len());

        let private_id = "avatar_snaggs_monday";
        let error = private_artwork(State(state.clone()), guest.clone(), Path(private_id.into()))
            .await
            .err()
            .unwrap();
        assert!(matches!(error, ApiError::Forbidden));
        let error = craft_cosmetic(
            State(state.clone()),
            guest,
            Json(CraftRequest {
                item_id: private_id.into(),
            }),
        )
        .await
        .err()
        .unwrap();
        assert!(matches!(error, ApiError::NotFound));
    }

    #[tokio::test]
    async fn members_get_private_artwork_from_the_configured_folder() {
        let mut state = state_with_users(&["alice"]).await;
        let dir = std::env::temp_dir().join(format!("coffee-private-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("avatar_snaggs_monday.webp"), b"RIFFfakewebp").unwrap();
        state.config.cosmetics_private_dir = dir.to_string_lossy().into_owned();

        let response = private_artwork(
            State(state.clone()),
            member("alice"),
            Path("avatar_snaggs_monday".into()),
        )
        .await
        .unwrap();
        assert_eq!(response.headers()[header::CONTENT_TYPE], "image/webp");

        // Public items and unknown ids are not served here (no path tricks either).
        for id in [
            "avatar_bean",
            "../avatar_snaggs_monday",
            "avatar_snaggs_brb",
        ] {
            let error = private_artwork(State(state.clone()), member("alice"), Path(id.into()))
                .await
                .err()
                .unwrap();
            assert!(matches!(error, ApiError::NotFound), "{id}");
        }
    }

    #[tokio::test]
    async fn activity_and_paid_tabs_earn_crates_after_onboarding() {
        let state = state_with_users(&["alice"]).await;
        let onboarded_at = ensure_onboarded(&state, "alice").await.unwrap();
        // A full lab week in March 2027 (Mon 1 .. Fri 5), five drinks a day.
        for day in 1..=5 {
            let start = chrono_tz::Europe::Berlin
                .with_ymd_and_hms(2027, 3, day, 9, 0, 0)
                .unwrap()
                .timestamp();
            add_coffees(&state, "alice", 5, start).await;
        }
        for (id, cents) in [("tab", 500), ("pennies", 50)] {
            sqlx::query(
                "INSERT INTO payments (id, user_id, amount_cents, drink_count, period_end_ts, paid_at, created_at)
                 VALUES (?, 'alice', ?, 1, ?, ?, ?)",
            )
            .bind(id)
            .bind(cents)
            .bind(onboarded_at + 10)
            .bind(onboarded_at + 10)
            .bind(onboarded_at + 10)
            .execute(&state.db)
            .await
            .unwrap();
        }

        let first = sync_grants(&state, "alice").await.unwrap();
        assert!(
            sync_grants(&state, "alice").await.unwrap().is_empty(),
            "idempotent"
        );
        let locker = load_locker(&state, "alice").await.unwrap();

        assert_eq!(locker.sources.daily, 1); // the 5th active day
        assert_eq!(locker.sources.streak, 1); // 5 lab days in a row
        assert_eq!(locker.sources.milestone, 1); // the 25th drink
        assert_eq!(locker.sources.stamp_card, 2); // 25 paid drinks in one month
        assert_eq!(locker.sources.payment, 1); // the 50 ct payment does not count
        assert_eq!(locker.sources.advent, 0);
        assert_eq!(first.len() as i64, locker.boxes_earned);
    }

    #[tokio::test]
    async fn split_payments_earn_one_crate_per_month() {
        let state = state_with_users(&["alice"]).await;
        let onboarded_at = ensure_onboarded(&state, "alice").await.unwrap();
        let next_month = onboarded_at + 40 * 24 * 3600;
        // Ten 1 EUR payments right after another, then one the month after.
        let paid_at = (0..10).map(|i| onboarded_at + i).chain([next_month]);
        for (i, at) in paid_at.enumerate() {
            sqlx::query(
                "INSERT INTO payments (id, user_id, amount_cents, drink_count, period_end_ts, paid_at, created_at)
                 VALUES (?, 'alice', 100, 2, ?, ?, ?)",
            )
            .bind(format!("pay-{i}"))
            .bind(at)
            .bind(at)
            .bind(at)
            .execute(&state.db)
            .await
            .unwrap();
        }
        sync_grants(&state, "alice").await.unwrap();
        assert_eq!(
            load_locker(&state, "alice").await.unwrap().sources.payment,
            2
        );
    }

    #[tokio::test]
    async fn logging_a_drink_reports_the_crates_it_earned() {
        let state = state_with_users(&["alice"]).await;
        ensure_onboarded(&state, "alice").await.unwrap();
        let (_, response) = crate::consumptions::create_consumption(
            State(state.clone()),
            member("alice"),
            Json(crate::consumptions::CreateConsumptionRequest {
                machine_id: "main".into(),
                beverage_id: "coffee".into(),
            }),
        )
        .await
        .unwrap();
        // One active day is not enough for an active-day crate (every 5th day).
        assert!(
            !response
                .crates_earned
                .iter()
                .any(|source| source == "daily")
        );
        // The first coffee unlocks an achievement, which is a crate too.
        assert!(
            response
                .crates_earned
                .iter()
                .any(|source| source == "achievement")
        );
    }

    #[tokio::test]
    async fn grants_are_skipped_once_the_reward_epoch_moved_on() {
        let state = state_with_users(&["alice"]).await;
        let stale = reward_epoch(&state, "alice").await.unwrap();
        // What undoing a drink does.
        sqlx::query("UPDATE users SET reward_epoch = reward_epoch + 1 WHERE id = 'alice'")
            .execute(&state.db)
            .await
            .unwrap();
        assert!(
            !grant_at_epoch(&state, "alice", "daily", "2026-11-05", 1, Some(stale))
                .await
                .unwrap()
        );
        let current = reward_epoch(&state, "alice").await.unwrap();
        assert!(
            grant_at_epoch(&state, "alice", "daily", "2026-11-05", 1, Some(current))
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn new_users_start_without_a_crate() {
        let state = state_with_users(&["bob"]).await;
        sync_grants(&state, "bob").await.unwrap();
        assert_eq!(load_locker(&state, "bob").await.unwrap().boxes_earned, 0);
    }

    #[tokio::test]
    async fn events_after_onboarding_earn_one_crate_each() {
        let state = state_with_users(&["alice"]).await;
        let onboarded_at = ensure_onboarded(&state, "alice").await.unwrap();
        let later = onboarded_at + 10;

        unlock_achievement_at(&state, "alice", "first_coffee", later).await;
        unlock_achievement_at(&state, "alice", "collector", later).await;
        add_expense(&state, "alice", "beans", 1999, later).await;
        add_expense(&state, "alice", "refund", -500, later).await;
        add_coffees(&state, "alice", 12, later).await;

        sync_grants(&state, "alice").await.unwrap();
        sync_grants(&state, "alice").await.unwrap();
        let locker = load_locker(&state, "alice").await.unwrap();

        assert_eq!(locker.sources.achievement, 2);
        assert_eq!(locker.sources.shopping, 1);
        assert_eq!(locker.sources.stamp_card, 1);
        assert_eq!(locker.sources.daily, 0); // all 12 coffees on one active day
        assert_eq!(locker.boxes_available, 4);
    }

    #[tokio::test]
    async fn opening_consumes_boxes_and_refuses_when_empty() {
        let state = state_with_users(&["alice"]).await;
        let error = open_lootbox(State(state.clone()), member("alice"))
            .await
            .err()
            .unwrap();
        assert!(matches!(error, ApiError::Conflict(_)));

        let onboarded_at = ensure_onboarded(&state, "alice").await.unwrap();
        unlock_achievement_at(&state, "alice", "first_coffee", onboarded_at + 1).await;

        let opened = open_lootbox(State(state.clone()), member("alice"))
            .await
            .unwrap()
            .0;
        assert!(opened.item.owned);
        assert!(!opened.duplicate);
        assert_eq!(opened.beans_awarded, 0);
        // Opening the first crate unlocks "Unboxing day", which grants the next one.
        assert!(
            opened
                .unlocked_achievements
                .iter()
                .any(|achievement| achievement.id == "unboxing_day")
        );
        assert_eq!(opened.locker.boxes_opened, 1);
        assert_eq!(opened.locker.boxes_available, 1);

        let second = open_lootbox(State(state.clone()), member("alice"))
            .await
            .unwrap()
            .0;
        assert_eq!(second.locker.boxes_available, 0);
        let error = open_lootbox(State(state.clone()), member("alice"))
            .await
            .err()
            .unwrap();
        assert!(matches!(error, ApiError::Conflict(_)));
    }

    #[tokio::test]
    async fn duplicates_turn_into_beans() {
        let state = state_with_users(&["alice"]).await;
        for item in CATALOG {
            give_item(&state, "alice", item.id).await;
        }
        unlock_achievement(&state, "alice", "first_coffee").await;

        let opened = open_lootbox(State(state.clone()), member("alice"))
            .await
            .unwrap()
            .0;
        let rarity = catalog::find(&opened.item.id).unwrap().rarity;
        assert!(opened.duplicate);
        assert_eq!(opened.beans_awarded, rarity.duplicate_beans());
        assert_eq!(opened.locker.beans, rarity.duplicate_beans());
    }

    #[tokio::test]
    async fn crafting_spends_beans_and_respects_rules() {
        let state = state_with_users(&["alice"]).await;
        let request = |item_id: &str| {
            Json(CraftRequest {
                item_id: item_id.into(),
            })
        };

        let error = craft_cosmetic(
            State(state.clone()),
            member("alice"),
            request("frame_crema"),
        )
        .await
        .err()
        .unwrap();
        assert!(matches!(error, ApiError::Conflict(message) if message == "not enough beans"));

        give_beans(&state, "alice", 100).await;
        let crafted = craft_cosmetic(
            State(state.clone()),
            member("alice"),
            request("frame_crema"),
        )
        .await
        .unwrap()
        .0;
        assert!(crafted.item.owned);
        assert_eq!(crafted.locker.beans, 100 - Rarity::Common.craft_cost());

        let error = craft_cosmetic(
            State(state.clone()),
            member("alice"),
            request("frame_crema"),
        )
        .await
        .err()
        .unwrap();
        assert!(
            matches!(error, ApiError::Conflict(message) if message == "you already own this item")
        );

        give_beans(&state, "alice", 5_000).await;
        let error = craft_cosmetic(
            State(state.clone()),
            member("alice"),
            request("collectible_kn_museum_ticket"),
        )
        .await
        .err()
        .unwrap();
        assert!(matches!(error, ApiError::BadRequest(_)));
    }

    #[tokio::test]
    async fn equipping_requires_ownership_and_matching_slot() {
        let state = state_with_users(&["alice"]).await;
        let equip = |slot: Slot, item_id: Option<&str>| {
            Json(EquipRequest {
                slot,
                item_id: item_id.map(Into::into),
            })
        };

        let error = equip_cosmetic(
            State(state.clone()),
            member("alice"),
            equip(Slot::Avatar, Some("avatar_bean")),
        )
        .await
        .err()
        .unwrap();
        assert!(matches!(error, ApiError::Forbidden));

        give_item(&state, "alice", "avatar_bean").await;
        give_item(&state, "alice", "title_barista").await;
        let error = equip_cosmetic(
            State(state.clone()),
            member("alice"),
            equip(Slot::Frame, Some("avatar_bean")),
        )
        .await
        .err()
        .unwrap();
        assert!(matches!(error, ApiError::BadRequest(_)));

        let _ = equip_cosmetic(
            State(state.clone()),
            member("alice"),
            equip(Slot::Avatar, Some("avatar_bean")),
        )
        .await
        .unwrap();
        let locker = equip_cosmetic(
            State(state.clone()),
            member("alice"),
            equip(Slot::Title, Some("title_barista")),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(locker.equipped.avatar.as_deref(), Some("avatar_bean"));
        assert!(
            locker
                .items
                .iter()
                .any(|item| item.id == "avatar_bean" && item.equipped)
        );

        let public = public_cosmetics_by_user(&state).await.unwrap();
        assert_eq!(
            public.get("alice"),
            Some(&PublicCosmetics {
                avatar: Some("avatar_bean".into()),
                avatar_artwork: Some("cosmetics/avatars/avatar_bean.webp".into()),
                frame: None,
                title: Some("Barista".into()),
            })
        );

        let locker = equip_cosmetic(
            State(state.clone()),
            member("alice"),
            equip(Slot::Avatar, None),
        )
        .await
        .unwrap()
        .0;
        assert_eq!(locker.equipped.avatar, None);
    }
}
