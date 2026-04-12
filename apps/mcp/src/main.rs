//! Historiador MCP server — standalone, read-only, externally exposed.
//!
//! # Invariant (ADR-003)
//!
//! This binary **must never** call `historiador_db::run_migrations` and
//! **must never** reference the `DATABASE_URL_READWRITE` environment
//! variable. Both rules are enforced by convention in this crate and by
//! the `historiador_mcp` Postgres role at the DB layer.

use std::sync::Arc;

use anyhow::Context;
use axum::{middleware, routing::{get, post}, Router};
use sha2::{Digest, Sha256};
use std::net::SocketAddr;
use tokio::signal;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use historiador_db::vector_store::InMemoryVectorStore;
use historiador_llm::{EmbeddingClient, OpenAiEmbeddingClient, StubEmbeddingClient};

mod auth;
mod health;
mod query;
mod state;

use state::McpState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Best-effort .env load so `cargo run` from the repo root picks up
    // local dev config without a wrapper script. Silently ignored if
    // the file is absent — production containers pass env vars directly.
    let _ = dotenvy::dotenv();

    init_tracing();

    // MCP receives the READONLY credential only.
    let database_url =
        std::env::var("DATABASE_URL_READONLY").context("DATABASE_URL_READONLY is required")?;
    let port: u16 = std::env::var("MCP_PORT")
        .unwrap_or_else(|_| "3002".to_string())
        .parse()
        .context("MCP_PORT must be a valid u16")?;

    // Bearer token for MCP auth — required in production.
    let bearer_token = std::env::var("MCP_BEARER_TOKEN")
        .unwrap_or_else(|_| {
            tracing::warn!("MCP_BEARER_TOKEN not set — using default dev token");
            "dev-mcp-token".to_string()
        });
    let bearer_token_hash: [u8; 32] = Sha256::digest(bearer_token.as_bytes()).into();

    // Build embedding client from env.
    let llm_provider = std::env::var("LLM_PROVIDER").unwrap_or_default();
    let llm_api_key = std::env::var("LLM_API_KEY").unwrap_or_default();

    let embedding_client: Arc<dyn EmbeddingClient> = match llm_provider.as_str() {
        "openai" if !llm_api_key.is_empty() => {
            tracing::info!("MCP embedding provider: OpenAI");
            Arc::new(OpenAiEmbeddingClient::new(&llm_api_key))
        }
        "anthropic" => {
            // Anthropic has no embedding API — check for EMBEDDING_API_KEY.
            match std::env::var("EMBEDDING_API_KEY") {
                Ok(key) if !key.is_empty() => {
                    tracing::info!("MCP embedding provider: OpenAI (via EMBEDDING_API_KEY)");
                    Arc::new(OpenAiEmbeddingClient::new(&key))
                }
                _ => {
                    tracing::info!("MCP embedding provider: stub (Anthropic has no embedding API)");
                    Arc::new(StubEmbeddingClient::default())
                }
            }
        }
        _ => {
            tracing::info!("MCP embedding provider: stub");
            Arc::new(StubEmbeddingClient::default())
        }
    };

    // Open the pool eagerly so credential failures surface at boot,
    // not on the first query. No migrations — see invariant above.
    let pool = historiador_db::connect(&database_url)
        .await
        .context("failed to connect to postgres as readonly role")?;

    let state = Arc::new(McpState {
        pool,
        vector_store: Arc::new(InMemoryVectorStore::new()),
        embedding_client,
        bearer_token_hash,
    });

    // Routes: /health is public, /query requires bearer token.
    let authed_routes = Router::new()
        .route("/query", post(query::handler))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth::bearer_auth,
        ));

    let app = Router::new()
        .route("/health", get(health::handler))
        .merge(authed_routes)
        .with_state(state)
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
