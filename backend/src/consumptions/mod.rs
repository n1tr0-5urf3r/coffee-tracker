use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, post},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    AppState,
    achievements::{evaluate_after_consumption, evaluate_global, revoke_unlocks_since},
    auth::CurrentUser,
    cosmetics::{revoke_grants_since, sync_grants},
    db::now_ts,
    error::{ApiError, ApiResult},
    notifications::notify_unlocks,
    stock::{decrement_for_beverage, restore_for_beverage},
    wheel::redeem_free_drink,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/consumptions", post(create_consumption))
        .route("/consumptions/{id}", delete(delete_consumption))
}

#[derive(Deserialize, ToSchema)]
pub struct CreateConsumptionRequest {
    pub machine_id: String,
    pub beverage_id: String,
}

#[derive(Serialize, ToSchema)]
pub struct ConsumptionResponse {
    pub id: String,
    pub user_id: String,
    pub machine_id: String,
    pub beverage_id: String,
    pub consumed_at: i64,
    /// Sources of the crates this drink just earned (e.g. `daily`, `streak`,
    /// `milestone`, `achievement`), one entry per crate.
    pub crates_earned: Vec<String>,
    /// Whether a free drink won on the lucky wheel paid for this one.
    pub free_drink_voucher_used: bool,
}

#[utoipa::path(
    post,
    path = "/api/v1/consumptions",
    request_body = CreateConsumptionRequest,
    responses((status = 201, body = ConsumptionResponse))
)]
pub(crate) async fn create_consumption(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(input): Json<CreateConsumptionRequest>,
) -> ApiResult<(StatusCode, Json<ConsumptionResponse>)> {
    ensure_exists(&state, "machines", &input.machine_id).await?;
    ensure_exists(&state, "beverages", &input.beverage_id).await?;

    let id = Uuid::new_v4().to_string();
    let cooldown = || {
        ApiError::Conflict(
            "last consumption for this user was logged less than five seconds ago".into(),
        )
    };
    // The first statement takes the user's write lock and checks the cooldown
    // in one go, so parallel POSTs cannot all pass it. `last_logged_at` is not
    // reset by an undo, so log + undo loops cannot skip the cooldown either.
    // The clock is read under the lock (`unixepoch()`), so a drink can't be
    // stamped before a payment's cutoff and committed after it.
    let mut tx = state.db.begin().await?;
    let stamped: Option<(i64,)> = sqlx::query_as(
        "UPDATE users SET last_logged_at = unixepoch()
         WHERE id = ? AND (last_logged_at IS NULL OR last_logged_at <= unixepoch() - ?)
         RETURNING last_logged_at",
    )
    .bind(&user.id)
    .bind(COOLDOWN_SECS)
    .fetch_optional(&mut *tx)
    .await?;
    let (now,) = stamped.ok_or_else(cooldown)?;
    let inserted = sqlx::query(
        "INSERT INTO consumptions (id, user_id, machine_id, beverage_id, consumed_at, created_at)
         SELECT ?, ?, ?, ?, ?, ?
         WHERE NOT EXISTS (
             SELECT 1 FROM consumptions WHERE user_id = ? AND consumed_at > ?
         )",
    )
    .bind(&id)
    .bind(&user.id)
    .bind(&input.machine_id)
    .bind(&input.beverage_id)
    .bind(now)
    .bind(now)
    .bind(&user.id)
    .bind(now - COOLDOWN_SECS)
    .execute(&mut *tx)
    .await?;
    if inserted.rows_affected() == 0 {
        return Err(cooldown());
    }
    tx.commit().await?;

    let free_drink_voucher_used =
        redeem_free_drink(&state, &user.id, &id, &input.beverage_id).await?;
    decrement_for_beverage(&state, &input.beverage_id).await?;
    // Activity crates first (daily, streak, Advent, milestone), so achievements
    // that count them see this drink; then the crates for new achievements.
    let mut crates_earned = sync_grants(&state, &user.id).await?;
    let unlocks = evaluate_after_consumption(&state, &user, &id, now).await?;
    notify_unlocks(&state, &user, &unlocks).await;
    if !unlocks.is_empty() {
        crates_earned.extend(sync_grants(&state, &user.id).await?);
    }

    Ok((
        StatusCode::CREATED,
        Json(ConsumptionResponse {
            id,
            user_id: user.id,
            machine_id: input.machine_id,
            beverage_id: input.beverage_id,
            consumed_at: now,
            crates_earned,
            free_drink_voucher_used,
        }),
    ))
}

/// Minimum time between two drinks of the same user.
const COOLDOWN_SECS: i64 = 5;
/// How long a drink can be undone after logging it.
const UNDO_WINDOW_SECS: i64 = 5 * 60;

/// Undoing a drink also takes back what it earned: every crate and achievement
/// since the drink was logged is revoked and then re-evaluated against the
/// remaining drinks. A drink cannot be undone once it is on a paid tab, nor once
/// a crate was opened after it, since that crate may have been one of its rewards.
#[utoipa::path(
    delete,
    path = "/api/v1/consumptions/{id}",
    params(("id" = String, Path)),
    responses(
        (status = 204, description = "Consumption deleted"),
        (status = 409, description = "The drink is on a paid tab, or a crate was opened since")
    )
)]
pub(crate) async fn delete_consumption(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let now = now_ts();
    let row: Option<(String, i64, String)> =
        sqlx::query_as("SELECT user_id, consumed_at, beverage_id FROM consumptions WHERE id = ?")
            .bind(&id)
            .fetch_optional(&state.db)
            .await?;
    let (owner_id, consumed_at, beverage_id) = row.ok_or(ApiError::NotFound)?;
    if owner_id != user.id {
        return Err(ApiError::Forbidden);
    }
    if now - consumed_at > UNDO_WINDOW_SECS {
        return Err(ApiError::Forbidden);
    }

    // The delete comes first, so the transaction holds the write lock from its
    // first statement: a concurrent crate opening lands either before it (and
    // blocks the undo) or after the revocation.
    let mut tx = state.db.begin().await?;
    let deleted = sqlx::query(
        "DELETE FROM consumptions
         WHERE id = ? AND user_id = ?
           AND NOT EXISTS (
               SELECT 1 FROM lootbox_openings WHERE user_id = ? AND opened_at >= ?
           )
           -- A payment bills every drink before its period end.
           AND NOT EXISTS (
               SELECT 1 FROM payments WHERE user_id = ? AND period_end_ts > ?
           )",
    )
    .bind(&id)
    .bind(&user.id)
    .bind(&user.id)
    .bind(consumed_at)
    .bind(&user.id)
    .bind(consumed_at)
    .execute(&mut *tx)
    .await?;
    if deleted.rows_affected() == 0 {
        let (still_there, paid): (bool, bool) = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM consumptions WHERE id = ?),
                    EXISTS(SELECT 1 FROM payments WHERE user_id = ? AND period_end_ts > ?)",
        )
        .bind(&id)
        .bind(&user.id)
        .bind(consumed_at)
        .fetch_one(&mut *tx)
        .await?;
        return Err(match (still_there, paid) {
            (false, _) => ApiError::NotFound,
            (true, true) => ApiError::Conflict(
                "this drink is already on a paid tab, so it can no longer be undone".into(),
            ),
            (true, false) => ApiError::Conflict(
                "a crate was opened since this drink was logged, so it can no longer be undone"
                    .into(),
            ),
        });
    }
    // Reward writes computed from data that still contained this drink are
    // skipped from now on (see `achievements::reward_epoch`).
    sqlx::query("UPDATE users SET reward_epoch = reward_epoch + 1 WHERE id = ?")
        .bind(&user.id)
        .execute(&mut *tx)
        .await?;
    revoke_unlocks_since(&mut tx, &user.id, consumed_at).await?;
    revoke_grants_since(&mut tx, &user.id, consumed_at).await?;
    // However the openings and revoked crates line up in time, the user must
    // not end up having opened more crates than they earned.
    let (overdrawn,): (bool,) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM lootbox_openings WHERE user_id = ?)
              > (SELECT COUNT(*) FROM lootbox_grants WHERE user_id = ?)",
    )
    .bind(&user.id)
    .bind(&user.id)
    .fetch_one(&mut *tx)
    .await?;
    if overdrawn {
        return Err(ApiError::Conflict(
            "a crate this drink earned was already opened, so it can no longer be undone".into(),
        ));
    }
    tx.commit().await?;

    if let Err(error) = restore_for_beverage(&state, &beverage_id).await {
        tracing::warn!(?error, "restoring stock after an undo failed");
    }
    // The undo is committed; a failure here must not look like a failed undo.
    // Retry, since everything restore does is idempotent.
    for attempt in 1..=3 {
        match restore_rewards_since(&state, &user, consumed_at, now).await {
            Ok(()) => break,
            Err(error) => tracing::warn!(?error, attempt, "restoring rewards after an undo failed"),
        }
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Re-earns what the user's remaining data still justifies after an undo
/// revoked everything since `since`, replaying the drinks logged since then in
/// order (mirroring `create_consumption`). Not notified again.
async fn restore_rewards_since(
    state: &AppState,
    user: &CurrentUser,
    since: i64,
    now: i64,
) -> ApiResult<()> {
    sync_grants(state, &user.id).await?;
    let later: Vec<(String, i64)> = sqlx::query_as(
        "SELECT id, consumed_at FROM consumptions
         WHERE user_id = ? AND consumed_at >= ?
         ORDER BY consumed_at ASC, id ASC",
    )
    .bind(&user.id)
    .bind(since)
    .fetch_all(&state.db)
    .await?;
    for (consumption_id, consumed_at) in later {
        evaluate_after_consumption(state, user, &consumption_id, consumed_at).await?;
    }
    evaluate_global(state, &user.id, now).await?;
    sync_grants(state, &user.id).await?;
    Ok(())
}

async fn ensure_exists(state: &AppState, table: &str, id: &str) -> ApiResult<()> {
    let sql = match table {
        "machines" => "SELECT id FROM machines WHERE id = ?",
        "beverages" => "SELECT id FROM beverages WHERE id = ?",
        _ => return Err(ApiError::BadRequest("unknown lookup table".into())),
    };
    let exists: Option<(String,)> = sqlx::query_as(sql)
        .bind(id)
        .fetch_optional(&state.db)
        .await?;
    exists.map(|_| ()).ok_or(ApiError::NotFound)
}

#[cfg(test)]
mod tests {
    use axum::{Json, extract::State};

    use super::*;
    use crate::cosmetics::{
        load_locker, open_lootbox,
        test_support::{member, state_with_users},
    };

    async fn log(state: &AppState, user: &str) -> ApiResult<ConsumptionResponse> {
        create_consumption(
            State(state.clone()),
            member(user),
            Json(CreateConsumptionRequest {
                machine_id: "main".into(),
                beverage_id: "coffee".into(),
            }),
        )
        .await
        .map(|(_, Json(response))| response)
    }

    async fn undo(state: &AppState, user: &str, id: &str) -> ApiResult<StatusCode> {
        delete_consumption(State(state.clone()), member(user), Path(id.into())).await
    }

    async fn unlocked(state: &AppState, user: &str) -> i64 {
        let (count,): (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM achievement_unlocks WHERE user_id = ?")
                .bind(user)
                .fetch_one(&state.db)
                .await
                .unwrap();
        count
    }

    /// Pretends the user's drinks and rewards all happened `secs` earlier.
    async fn backdate(state: &AppState, user: &str, secs: i64) {
        for sql in [
            "UPDATE consumptions SET consumed_at = consumed_at - ?1, created_at = created_at - ?1
             WHERE user_id = ?2",
            "UPDATE achievement_unlocks SET unlocked_at = unlocked_at - ?1 WHERE user_id = ?2",
            "UPDATE lootbox_grants SET granted_at = granted_at - ?1 WHERE user_id = ?2",
            "UPDATE users SET last_logged_at = last_logged_at - ?1 WHERE id = ?2",
        ] {
            sqlx::query(sql)
                .bind(secs)
                .bind(user)
                .execute(&state.db)
                .await
                .unwrap();
        }
    }

    #[tokio::test]
    async fn concurrent_logs_cannot_bypass_the_cooldown() {
        let state = state_with_users(&["alice"]).await;
        // Onboard first so the burst only races on the cooldown itself.
        sync_grants(&state, "alice").await.unwrap();
        let attempts = (0..15).map(|_| {
            let state = state.clone();
            tokio::spawn(async move { log(&state, "alice").await.is_ok() })
        });
        let mut accepted = 0;
        for attempt in attempts {
            accepted += attempt.await.unwrap() as i32;
        }
        assert_eq!(accepted, 1);
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM consumptions")
            .fetch_one(&state.db)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn undo_takes_back_the_rewards_of_the_drink() {
        let state = state_with_users(&["alice"]).await;
        let drink = log(&state, "alice").await.unwrap();
        // The first coffee unlocks an achievement, which earns a crate.
        assert!(!drink.crates_earned.is_empty());
        assert!(unlocked(&state, "alice").await > 0);

        assert_eq!(
            undo(&state, "alice", &drink.id).await.unwrap(),
            StatusCode::NO_CONTENT
        );
        assert_eq!(unlocked(&state, "alice").await, 0);
        let locker = load_locker(&state, "alice").await.unwrap();
        assert_eq!(locker.boxes_earned, 0);
        // Reading the locker re-syncs; the crate must not come back.
        sync_grants(&state, "alice").await.unwrap();
        assert_eq!(load_locker(&state, "alice").await.unwrap().boxes_earned, 0);
    }

    #[tokio::test]
    async fn undo_keeps_rewards_of_earlier_drinks() {
        let state = state_with_users(&["alice"]).await;
        log(&state, "alice").await.unwrap();
        backdate(&state, "alice", 60).await;
        let achievements = unlocked(&state, "alice").await;
        let crates = load_locker(&state, "alice").await.unwrap().boxes_earned;
        assert!(achievements > 0 && crates > 0);

        let second = log(&state, "alice").await.unwrap();
        undo(&state, "alice", &second.id).await.unwrap();
        assert_eq!(unlocked(&state, "alice").await, achievements);
        assert_eq!(
            load_locker(&state, "alice").await.unwrap().boxes_earned,
            crates
        );
    }

    async fn beans(state: &AppState) -> f64 {
        let (amount,): (f64,) =
            sqlx::query_as("SELECT current_amount FROM stock_items WHERE id = 'beans'")
                .fetch_one(&state.db)
                .await
                .unwrap();
        amount
    }

    #[tokio::test]
    async fn undo_restores_stock_and_keeps_the_cooldown() {
        let state = state_with_users(&["alice"]).await;
        sqlx::query(
            "UPDATE stock_items SET current_amount = 100, decrement_per_log = 1 WHERE id = 'beans'",
        )
        .execute(&state.db)
        .await
        .unwrap();
        let drink = log(&state, "alice").await.unwrap();
        assert_eq!(beans(&state).await, 99.0);
        undo(&state, "alice", &drink.id).await.unwrap();
        assert_eq!(beans(&state).await, 100.0);
        // Undoing does not reset the cooldown, so log + undo can't loop.
        assert!(matches!(
            log(&state, "alice").await,
            Err(ApiError::Conflict(_))
        ));
    }

    #[tokio::test]
    async fn paid_drinks_cannot_be_undone() {
        let state = state_with_users(&["alice"]).await;
        let drink = log(&state, "alice").await.unwrap();
        sqlx::query(
            "INSERT INTO payments (id, user_id, amount_cents, drink_count, period_end_ts, paid_at, created_at)
             VALUES ('pay', 'alice', 50, 1, ?1, ?1, ?1)",
        )
        .bind(drink.consumed_at + 1)
        .execute(&state.db)
        .await
        .unwrap();

        assert!(matches!(
            undo(&state, "alice", &drink.id).await,
            Err(ApiError::Conflict(message)) if message.contains("paid tab")
        ));
    }

    #[tokio::test]
    async fn opening_a_crate_blocks_the_undo() {
        let state = state_with_users(&["alice"]).await;
        let drink = log(&state, "alice").await.unwrap();
        let _ = open_lootbox(State(state.clone()), member("alice"))
            .await
            .unwrap();

        assert!(matches!(
            undo(&state, "alice", &drink.id).await,
            Err(ApiError::Conflict(_))
        ));
        // Nothing changed: the drink (and its debt) and the crate both stay.
        let (count,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM consumptions")
            .fetch_one(&state.db)
            .await
            .unwrap();
        assert_eq!(count, 1);
        assert!(load_locker(&state, "alice").await.unwrap().boxes_earned > 0);
    }
}
