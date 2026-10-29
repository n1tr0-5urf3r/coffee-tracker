pub mod achievements;
pub mod auth;
pub mod catalog;
pub mod config;
pub mod consumptions;
pub mod db;
pub mod error;
pub mod expenses;
pub mod loyalty;
pub mod notifications;
pub mod openapi;
pub mod payments;
pub mod social;
pub mod stats;
pub mod stock;
pub mod version;

use axum::{
    Router,
    http::{HeaderName, HeaderValue, Method, header},
};
use sqlx::SqlitePool;
use tower_cookies::CookieManagerLayer;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::config::Config;
use crate::db::seed_defaults;

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub db: SqlitePool,
    pub http: reqwest::Client,
}

pub async fn build_app(config: Config, db: SqlitePool) -> anyhow::Result<Router> {
    seed_defaults(&db).await?;

    let cors_origin = HeaderValue::from_str(&config.frontend_base_url)?;

    let state = AppState {
        config,
        db,
        http: reqwest::Client::new(),
    };

    if let Some(auto_payment_reminder) = state.config.auto_payment_reminder.clone() {
        payments::scheduler::spawn(state.clone(), auto_payment_reminder);
    }

    let api = Router::new()
        .merge(auth::router())
        .merge(catalog::router())
        .merge(consumptions::router())
        .merge(stats::router())
        .merge(loyalty::router())
        .merge(achievements::router())
        .merge(social::router())
        .merge(payments::router())
        .merge(expenses::router())
        .merge(stock::router())
        .merge(version::router());

    Ok(Router::new()
        .nest("/api/v1", api)
        .merge(openapi::router())
        .layer(CookieManagerLayer::new())
        .layer(
            CorsLayer::new()
                .allow_origin(cors_origin)
                .allow_methods([
                    Method::GET,
                    Method::POST,
                    Method::PATCH,
                    Method::DELETE,
                    Method::OPTIONS,
                ])
                .allow_headers([
                    header::CONTENT_TYPE,
                    header::ACCEPT,
                    header::AUTHORIZATION,
                    HeaderName::from_static("x-requested-with"),
                ])
                .allow_credentials(true),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state))
}
