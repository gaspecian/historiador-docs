use anyhow::Context;
use axum::{routing::get, Router};
use historiador_api::{health, routes, state::AppState};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::signal;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    // --- env vars (manual reads; revisit if the count grows past ~10) ---
    let database_url =
        std::env::var("DATABASE_URL_READWRITE").context("DATABASE_URL_READWRITE is required")?;
    let port: u16 = std::env::var("API_PORT")
        .unwrap_or_else(|_| "3001".to_string())
        .parse()
        .context("API_PORT must be a valid u16")?;
    let git_sha = std::env::var("GIT_SHA").unwrap_or_else(|_| "unknown".to_string());

    // --- database pool + migrations (api is the only service that migrates) ---
    let pool = historiador_db::connect(&database_url)
        .await
        .context("failed to connect to postgres")?;
    historiador_db::run_migrations(&pool)
        .await
        .context("failed to run migrations")?;
    tracing::info!("migrations applied");

    let state = Arc::new(AppState { pool, git_sha });

    let app = Router::new()
        .route("/health", get(health::handler))
        .nest("/auth", routes::auth_router())
        .nest("/pages", routes::pages_router())
        .nest("/collections", routes::collections_router())
        .nest("/admin", routes::admin_router())
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!(%addr, "api server listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("historiador_api=info,tower_http=info,sqlx=warn"));

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().with_target(true))
        .init();
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install ctrl_c handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install sigterm handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => { tracing::info!("ctrl_c received, shutting down"); }
        _ = terminate => { tracing::info!("sigterm received, shutting down"); }
    }
}
