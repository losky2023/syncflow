mod auth;
mod config;
mod device;
mod signal;
mod stun;

use axum::{
    routing::{get, post},
    Router,
};
use config::ServerConfig;
use signal::{DeviceRegistry, SignalState};
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub config: Arc<ServerConfig>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            "syncflow_server=debug,tower_http=debug",
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = ServerConfig::from_env();
    tracing::info!("Starting signal server on {}:{}", config.host, config.port);

    tracing::info!("Connecting to database at {}", config.database_url);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&config.database_url)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;
    tracing::info!("Database migrations complete");

    let app_state = AppState {
        pool,
        config: Arc::new(config),
    };

    let registry: DeviceRegistry = Arc::new(RwLock::new(HashMap::new()));
    let signal_state = SignalState {
        app: app_state.clone(),
        registry,
    };

    let app = Router::new()
        .route("/api/auth/register", post(auth::register))
        .route("/api/auth/login", post(auth::login))
        .route("/api/device/register", post(device::register_device))
        .route("/api/device/list", get(device::list_devices))
        .route("/ws/signal", get(signal::ws_handler))
        .route("/api/stun/config", post(stun::get_config))
        .with_state(signal_state);

    let addr = format!("{}:{}", app_state.config.host, app_state.config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Listening on {}", addr);

    axum::serve(listener, app).await?;
    Ok(())
}
