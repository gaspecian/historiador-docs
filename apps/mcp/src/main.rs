//! Historiador MCP server — standalone, read-only, externally exposed.
//!
//! # Invariant (ADR-003)
//!
//! This binary **must never** call `historiador_db::run_migrations` and
//! **must never** reference the `DATABASE_URL_READWRITE` environment
//! variable. Both rules are enforced by convention in this crate and by
//! the `historiador_mcp` Postgres role at the DB layer.

use anyhow::Context;
use axum::{routing::get, Router};
use std::net::SocketAddr;
use tokio::signal;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

mod health;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();

    // MCP receives the READONLY credential only.
    let database_url =
        std::env::var("DATABASE_URL_READONLY").context("DATABASE_URL_READONLY is required")?;
    let port: u16 = std::env::var("MCP_PORT")
        .unwrap_or_else(|_| "3002".to_string())
        .parse()
        .context("MCP_PORT must be a valid u16")?;

    // Open the pool eagerly so credential failures surface at boot,
    // not on the first query. No migrations — see invariant above.
    let _pool = historiador_db::connect(&database_url)
        .await
        .context("failed to connect to postgres as readonly role")?;

    let app = Router::new()
        .route("/health", get(health::handler))
        // TODO(Sprint 2): POST /mcp implementing the Model Context Protocol
        .layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!(%addr, "mcp server listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("historiador_mcp=info,tower_http=info"));

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
