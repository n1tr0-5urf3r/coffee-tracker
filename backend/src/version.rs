use axum::{Json, Router, routing::get};
use serde::Serialize;
use utoipa::ToSchema;

use crate::AppState;

pub fn router() -> Router<AppState> {
    Router::new().route("/version", get(version))
}

#[derive(Serialize, ToSchema)]
pub struct VersionResponse {
    pub backend_version: &'static str,
}

#[utoipa::path(
    get,
    path = "/api/v1/version",
    responses((status = 200, body = VersionResponse))
)]
pub(crate) async fn version() -> Json<VersionResponse> {
    Json(VersionResponse {
        backend_version: env!("CARGO_PKG_VERSION"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn version_endpoint_uses_package_version() {
        let Json(response) = version().await;
        assert_eq!(response.backend_version, "1.4.0");
    }
}
