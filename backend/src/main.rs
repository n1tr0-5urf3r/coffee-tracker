use coffee_tracker::{build_app, config::Config, db::connect};
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "coffee_tracker=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Config::from_env()?;
    let db = connect(&config.database_url).await?;
    let app = build_app(config.clone(), db).await?;
    let listener = TcpListener::bind(&config.bind_addr).await?;

    tracing::info!("listening on {}", config.bind_addr);
    axum::serve(listener, app).await?;
    Ok(())
}
