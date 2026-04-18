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
use axum::{
    middleware,
    routing::{get, post},
    Router,
};
use sha2::{Digest, Sha256};
use std::net::SocketAddr;
use tokio::signal;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use historiador_db::{
    chronik::{ChronikClient, ChronikConfig},
    vector_store::{ChronikVectorStore, InMemoryVectorStore, VectorStore},
};
use historiador_llm::{
    EmbeddingClient, OllamaEmbeddingClient, OpenAiEmbeddingClient, StubEmbeddingClient,
};

mod application;
mod auth;
mod health;
mod infrastructure;
mod jsonrpc;
mod query;
mod state;

use application::SearchChunksUseCase;
use infrastructure::PostgresChunkMetadataReader;
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
    let bearer_token = std::env::var("MCP_BEARER_TOKEN").unwrap_or_else(|_| {
        tracing::warn!("MCP_BEARER_TOKEN not set — using default dev token");
        "dev-mcp-token".to_string()
    });
    let bearer_token_hash: [u8; 32] = Sha256::digest(bearer_token.as_bytes()).into();

    // Open the pool eagerly so credential failures surface at boot,
    // not on the first query. No migrations — see invariant above.
    let pool = historiador_db::connect(&database_url)
        .await
        .context("failed to connect to postgres as readonly role")?;

    // Build embedding client. MCP reads the workspace row (read-only
    // role covers `workspaces`) to pick up the chosen provider and
    // embedding model. For cloud providers the encrypted key is not
    // accessible from MCP (ADR-003), so we still need LLM_API_KEY /
    // EMBEDDING_API_KEY in the environment for OpenAI / Anthropic. For
    // Ollama, the base URL is stored in the clear and no env var is
    // required.
    let workspace_row = historiador_db::postgres::workspaces::find_singleton(&pool).await?;
    let embedding_client: Arc<dyn EmbeddingClient> = match workspace_row.as_ref() {
        Some(ws) if ws.llm_provider == "ollama" => {
            let base = ws
                .llm_base_url
                .as_deref()
                .unwrap_or("http://localhost:11434");
            tracing::info!(
                base,
                model = ws.embedding_model.as_str(),
                "MCP embedding provider: Ollama"
            );
            Arc::new(OllamaEmbeddingClient::new(base, &ws.embedding_model))
        }
        Some(ws) if ws.llm_provider == "openai" || ws.llm_provider == "anthropic" => {
            let key = std::env::var("EMBEDDING_API_KEY")
                .ok()
                .or_else(|| std::env::var("LLM_API_KEY").ok())
                .unwrap_or_default();
            if key.is_empty() {
                tracing::warn!(
                    provider = ws.llm_provider.as_str(),
                    "no EMBEDDING_API_KEY / LLM_API_KEY set — MCP embedding falling back to stub"
                );
                Arc::new(StubEmbeddingClient::default())
            } else {
                tracing::info!(
                    model = ws.embedding_model.as_str(),
                    "MCP embedding provider: OpenAI"
                );
                Arc::new(OpenAiEmbeddingClient::with_model(
                    &key,
                    &ws.embedding_model,
                    1536,
                ))
            }
        }
        _ => {
            tracing::info!("MCP embedding provider: stub (setup not complete / test provider)");
            Arc::new(StubEmbeddingClient::default())
        }
    };

    // Build vector store: Chronik if configured, else bail unless
    // ALLOW_IN_MEMORY_VECTOR_STORE=true (code review finding 4.4).
    let chronik_url = std::env::var("CHRONIK_SQL_URL").ok();
    let allow_in_memory = historiador_db::vector_store::allow_in_memory_vector_store();
    let vector_store: Arc<dyn VectorStore> = match &chronik_url {
        Some(url) if !url.is_empty() => {
            let search_url = std::env::var("CHRONIK_SEARCH_URL").unwrap_or_else(|_| url.clone());
            match ChronikClient::new(ChronikConfig {
                base_url: url.clone(),
                search_base_url: search_url,
            }) {
                Ok(client) => {
                    tracing::info!("MCP vector store: Chronik-Stream");
                    Arc::new(ChronikVectorStore::new(client))
                }
                Err(e) if allow_in_memory => {
                    tracing::warn!(
                        error = %e,
                        "⚠️  Chronik init failed — MCP falling back to in-memory vector \
                         store because ALLOW_IN_MEMORY_VECTOR_STORE=true. \
                         DATA WILL BE LOST ON RESTART. Do not use this in production."
                    );
                    Arc::new(InMemoryVectorStore::new())
                }
                Err(e) => {
                    anyhow::bail!(
                        "MCP: Chronik init failed ({e}). Start Chronik \
                         (`docker compose up -d chronik`) or set \
                         ALLOW_IN_MEMORY_VECTOR_STORE=true for dev-only \
                         in-memory fallback (data lost on restart)."
                    );
                }
            }
        }
        _ if allow_in_memory => {
            tracing::warn!(
                "⚠️  MCP: CHRONIK_SQL_URL not set — using in-memory vector store \
                 because ALLOW_IN_MEMORY_VECTOR_STORE=true. \
                 DATA WILL BE LOST ON RESTART. Do not use this in production."
            );
            Arc::new(InMemoryVectorStore::new())
        }
        _ => {
            anyhow::bail!(
                "MCP: CHRONIK_SQL_URL is not set and ALLOW_IN_MEMORY_VECTOR_STORE \
                 is not true. Start Chronik (`docker compose up -d chronik`) or \
                 set ALLOW_IN_MEMORY_VECTOR_STORE=true for dev-only in-memory \
                 fallback (data lost on restart)."
            );
        }
    };

    // Reuse the workspace row loaded above for the embedding client.
    let workspace_id = workspace_row.as_ref().map(|w| w.id).unwrap_or_else(|| {
        tracing::warn!("no workspace found — MCP query logging will use nil UUID");
        uuid::Uuid::nil()
    });

    let internal_api_url =
        std::env::var("API_INTERNAL_URL").unwrap_or_else(|_| "http://localhost:3001".to_string());

    let metadata_reader = Arc::new(PostgresChunkMetadataReader::new(pool));
    let search_chunks = Arc::new(SearchChunksUseCase::new(
        embedding_client,
        vector_store,
        metadata_reader,
    ));

    let state = Arc::new(McpState {
        search_chunks,
        bearer_token_hash,
        internal_api_url,
        workspace_id,
    });

    // Routes: /health is public; /mcp (JSON-RPC 2.0 MCP protocol) and
    // /query (internal custom REST alias, kept for the web UI) both
    // require bearer token.
    let authed_routes = Router::new()
        .route("/mcp", post(jsonrpc::handler))
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
