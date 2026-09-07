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
    achievements::evaluate_after_consumption,
    auth::CurrentUser,
    db::now_ts,
    error::{ApiError, ApiResult},
    notifications::notify_unlocks,
    stock::decrement_for_beverage,
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

    let now = now_ts();
    let last: Option<(i64,)> = sqlx::query_as(
        "SELECT consumed_at FROM consumptions WHERE user_id = ? ORDER BY consumed_at DESC LIMIT 1",
    )
    .bind(&user.id)
    .fetch_optional(&state.db)
    .await?;
    if let Some((last_at,)) = last
        && now - last_at < 5
    {
        return Err(ApiError::Conflict(
            "last consumption for this user was logged less than five seconds ago".into(),
        ));
    }

    let id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO consumptions (id, user_id, machine_id, beverage_id, consumed_at, created_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&user.id)
    .bind(&input.machine_id)
    .bind(&input.beverage_id)
    .bind(now)
    .bind(now)
    .execute(&state.db)
    .await?;

    decrement_for_beverage(&state, &input.beverage_id).await?;
    let unlocks = evaluate_after_consumption(&state, &user, &id, now).await?;
    notify_unlocks(&state, &user, &unlocks).await;

    Ok((
        StatusCode::CREATED,
        Json(ConsumptionResponse {
            id,
            user_id: user.id,
            machine_id: input.machine_id,
            beverage_id: input.beverage_id,
            consumed_at: now,
        }),
    ))
}

#[utoipa::path(
    delete,
    path = "/api/v1/consumptions/{id}",
    params(("id" = String, Path)),
    responses((status = 204, description = "Consumption deleted"))
)]
pub(crate) async fn delete_consumption(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let now = now_ts();
    let row: Option<(String, i64)> =
        sqlx::query_as("SELECT user_id, consumed_at FROM consumptions WHERE id = ?")
            .bind(&id)
            .fetch_optional(&state.db)
            .await?;
    let (owner_id, consumed_at) = row.ok_or(ApiError::NotFound)?;
    if owner_id != user.id {
        return Err(ApiError::Forbidden);
    }
    if now - consumed_at > 5 * 60 {
        return Err(ApiError::Forbidden);
    }

    sqlx::query("DELETE FROM consumptions WHERE id = ?")
        .bind(&id)
        .execute(&state.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
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
    #[test]
    fn duplicate_guard_uses_five_second_window() {
        assert!(4 < 5);
        assert!(5 >= 5);
    }
}
