use axum::{Json, Router, extract::State, routing::get};
use serde::Serialize;
use utoipa::ToSchema;

use crate::{
    AppState,
    error::{ApiError, ApiResult},
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/machines", get(list_machines))
        .route("/beverages", get(list_beverages))
}

#[derive(Serialize, ToSchema, sqlx::FromRow)]
pub struct Machine {
    pub id: String,
    pub name: String,
    pub location: Option<String>,
}

#[derive(Serialize, ToSchema, sqlx::FromRow)]
pub struct Beverage {
    pub id: String,
    pub name: String,
    pub price_cents: i64,
    pub caffeine_mg: i64,
}

#[derive(Serialize, ToSchema)]
pub struct MachinesResponse {
    pub machines: Vec<Machine>,
}

#[derive(Serialize, ToSchema)]
pub struct BeveragesResponse {
    pub beverages: Vec<Beverage>,
}

#[utoipa::path(
    get,
    path = "/api/v1/machines",
    responses((status = 200, body = MachinesResponse))
)]
pub(crate) async fn list_machines(
    State(state): State<AppState>,
) -> ApiResult<Json<MachinesResponse>> {
    let machines =
        sqlx::query_as::<_, Machine>("SELECT id, name, location FROM machines ORDER BY name ASC")
            .fetch_all(&state.db)
            .await
            .map_err(ApiError::from)?;

    Ok(Json(MachinesResponse { machines }))
}

#[utoipa::path(
    get,
    path = "/api/v1/beverages",
    responses((status = 200, body = BeveragesResponse))
)]
pub(crate) async fn list_beverages(
    State(state): State<AppState>,
) -> ApiResult<Json<BeveragesResponse>> {
    let beverages = sqlx::query_as::<_, Beverage>(
        "SELECT id, name, price_cents, caffeine_mg
         FROM beverages
         ORDER BY CASE id WHEN 'coffee' THEN 0 WHEN 'espresso_pads' THEN 1 WHEN 'eis' THEN 2 WHEN 'tea' THEN 3 ELSE 4 END,
                  price_cents ASC, name ASC",
    )
    .fetch_all(&state.db)
    .await
    .map_err(ApiError::from)?;

    Ok(Json(BeveragesResponse { beverages }))
}

#[cfg(test)]
mod tests {
    use axum::extract::State;
    use uuid::Uuid;

    use super::*;
    use crate::{config::Config, db::connect};

    async fn state() -> AppState {
        let db_path = format!("/tmp/coffee-catalog-{}.db", Uuid::new_v4());
        let db = connect(&format!("sqlite://{db_path}")).await.unwrap();
        crate::db::seed_defaults(&db).await.unwrap();
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

    #[tokio::test]
    async fn catalog_orders_primary_beverages_then_eis() {
        let state = state().await;
        let Json(response) = list_beverages(State(state)).await.unwrap();
        let ids = response
            .beverages
            .iter()
            .map(|beverage| beverage.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(&ids[..4], ["coffee", "espresso_pads", "eis", "tea"]);
        let eis = response
            .beverages
            .iter()
            .find(|beverage| beverage.id == "eis")
            .unwrap();
        assert_eq!(eis.name, "Eis");
        assert_eq!(eis.price_cents, 50);
        assert_eq!(eis.caffeine_mg, 0);
        let tea = response
            .beverages
            .iter()
            .find(|beverage| beverage.id == "tea")
            .unwrap();
        assert_eq!(tea.name, "Tea");
        assert_eq!(tea.price_cents, 25);
        assert_eq!(tea.caffeine_mg, 0);
    }
}
