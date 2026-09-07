use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("authentication required")]
    Unauthorized,
    #[error("access denied")]
    Forbidden,
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Conflict(String),
    #[error("not found")]
    NotFound,
    #[error("{0}")]
    ServiceUnavailable(String),
    #[error("upstream service error")]
    Upstream,
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self {
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::ServiceUnavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::Upstream => StatusCode::BAD_GATEWAY,
            Self::Sqlx(_) | Self::Other(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let message = if status == StatusCode::INTERNAL_SERVER_ERROR {
            "internal server error".to_string()
        } else {
            self.to_string()
        };
        (status, Json(ErrorBody { error: message })).into_response()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
