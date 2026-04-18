use std::net::SocketAddr;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use anyhow::Context;
use historiador_api::{
    app,
    crypto::Cipher,
    presentation::{BuildDeps, UseCases},
    setup::llm_probe::HttpLlmProbe,
    state::AppState,
};
use historiador_db::{
    chronik::{ChronikClient, ChronikConfig},
    postgres::installation,
    vector_store::{ChronikVectorStore, InMemoryVectorStore, VectorStore},
};
use historiador_llm::{
    AnthropicTextGenerationClient, EmbeddingClient, OllamaEmbeddingClient, OllamaTextClient,
    OpenAiEmbeddingClient, OpenAiTextGenerationClient, StubEmbeddingClient,
    StubTextGenerationClient, TextGenerationClient,
};
use tokio::signal;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Best-effort .env load so `cargo run` from the repo root picks up
    // local dev config without a wrapper script. Silently ignored if
    // the file is absent — production containers pass env vars directly.
    let _ = dotenvy::dotenv();

    init_tracing();

    // --- env vars (manual reads; revisit if the count grows past ~10) ---
    let database_url =
        std::env::var("DATABASE_URL_READWRITE").context("DATABASE_URL_READWRITE is required")?;
    let port: u16 = std::env::var("API_PORT")
        .unwrap_or_else(|_| "3001".to_string())
        .parse()
        .context("API_PORT must be a valid u16")?;
    let git_sha = std::env::var("GIT_SHA").unwrap_or_else(|_| "unknown".to_string());

    // Secrets: fail-fast if either is missing or malformed. We would
    // rather a crash at boot than a silent security downgrade.
    let jwt_secret = std::env::var("JWT_SECRET").context("JWT_SECRET is required")?;
    if jwt_secret.len() < 32 {
        anyhow::bail!("JWT_SECRET must be at least 32 characters");
    }

    let encryption_key_b64 = std::env::var("APP_ENCRYPTION_KEY")
        .context("APP_ENCRYPTION_KEY is required (base64 32 bytes)")?;
    let cipher = Cipher::from_base64(&encryption_key_b64)?;

    let public_base_url =
        std::env::var("PUBLIC_BASE_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());

    // --- database pool + migrations (api is the only service that migrates) ---
    let pool = historiador_db::connect(&database_url)
        .await
        .context("failed to connect to postgres")?;
    historiador_db::run_migrations(&pool)
        .await
        .context("failed to run migrations")?;
    tracing::info!("migrations applied");

    // Seed the setup-complete flag from the installation row.
    let install = installation::get(&pool).await?;
    let setup_complete = AtomicBool::new(install.setup_complete);
    tracing::info!(
        setup_complete = install.setup_complete,
        "installation loaded"
    );

    // Build LLM clients. When setup has already run, the stored
    // workspace row is the source of truth (provider, models, and
    // Ollama base URL or encrypted key). Before setup completes —
    // e.g. the first boot in a fresh deploy — fall back to env vars
    // so the setup wizard itself has a working probe/draft surface.
    let workspace_row = historiador_db::postgres::workspaces::find_singleton(&pool).await?;
    let (embedding_client, text_generation_client) =
        build_llm_clients_from_workspace(&cipher, workspace_row.as_ref())?;

    // Build Chronik client if configured; fall back to InMemoryVectorStore.
    let chronik_url = std::env::var("CHRONIK_SQL_URL").ok();
    let (vector_store, chronik): (Arc<dyn VectorStore>, Option<ChronikClient>) = match chronik_url {
        Some(url) if !url.is_empty() => {
            let search_url = std::env::var("CHRONIK_SEARCH_URL").unwrap_or_else(|_| url.clone());

            match ChronikClient::new(ChronikConfig {
                base_url: url,
                search_base_url: search_url,
            }) {
                Ok(client) => {
                    tracing::info!("vector store: Chronik-Stream");
                    let vs = Arc::new(ChronikVectorStore::new(client.clone()));
                    (vs, Some(client))
                }
                Err(e) => {
                    tracing::warn!(error = %e, "failed to init Chronik — falling back to in-memory");
                    (Arc::new(InMemoryVectorStore::new()), None)
                }
            }
        }
        _ => {
            tracing::info!("vector store: in-memory (CHRONIK_SQL_URL not set)");
            (Arc::new(InMemoryVectorStore::new()), None)
        }
    };

    let llm_probe: Arc<dyn historiador_api::setup::llm_probe::LlmProbe> =
        Arc::new(HttpLlmProbe::default());
    let jwt_secret_bytes = jwt_secret.into_bytes();

    let use_cases = Arc::new(UseCases::build(BuildDeps {
        pool: pool.clone(),
        cipher: cipher.clone(),
        jwt_secret: jwt_secret_bytes.clone(),
        llm_probe: llm_probe.clone(),
        vector_store: vector_store.clone(),
        embedding_client: embedding_client.clone(),
        text_generation_client: text_generation_client.clone(),
        chronik: chronik.clone(),
    }));

    let state = Arc::new(AppState {
        pool,
        git_sha,
        jwt_secret: jwt_secret_bytes,
        cipher,
        public_base_url,
        setup_complete,
        llm_probe,
        vector_store,
        embedding_client,
        text_generation_client,
        chronik,
        use_cases,
    });

    let app = app::build_router(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!(%addr, "api server listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

/// Build the LLM client pair from a workspace row, falling back to env
/// vars before setup has completed. Kept here (rather than in the
/// `historiador_llm` crate) because it handles `api`-specific concerns:
/// decrypting the stored key via the `Cipher` and reading env vars.
fn build_llm_clients_from_workspace(
    cipher: &Cipher,
    workspace: Option<&historiador_db::postgres::workspaces::Workspace>,
) -> anyhow::Result<(Arc<dyn EmbeddingClient>, Arc<dyn TextGenerationClient>)> {
    // Pre-setup: honor legacy LLM_PROVIDER + LLM_API_KEY env vars.
    let Some(ws) = workspace else {
        let provider = std::env::var("LLM_PROVIDER").unwrap_or_default();
        let api_key = std::env::var("LLM_API_KEY").unwrap_or_default();
        return Ok(match provider.as_str() {
            "openai" if !api_key.is_empty() => {
                tracing::info!("LLM provider (env): OpenAI");
                (
                    Arc::new(OpenAiEmbeddingClient::new(&api_key)),
                    Arc::new(OpenAiTextGenerationClient::new(&api_key)),
                )
            }
            "anthropic" if !api_key.is_empty() => {
                tracing::info!("LLM provider (env): Anthropic");
                let emb: Arc<dyn EmbeddingClient> = match std::env::var("EMBEDDING_API_KEY") {
                    Ok(k) if !k.is_empty() => Arc::new(OpenAiEmbeddingClient::new(&k)),
                    _ => Arc::new(StubEmbeddingClient::default()),
                };
                (emb, Arc::new(AnthropicTextGenerationClient::new(&api_key)))
            }
            _ => {
                tracing::info!("LLM provider (env): stub — setup not complete");
                (
                    Arc::new(StubEmbeddingClient::default()),
                    Arc::new(StubTextGenerationClient),
                )
            }
        });
    };

    // Post-setup: every field comes from the workspace row.
    let gen_model = ws.generation_model.as_str();
    let embed_model = ws.embedding_model.as_str();

    let pair: (Arc<dyn EmbeddingClient>, Arc<dyn TextGenerationClient>) =
        match ws.llm_provider.as_str() {
            "openai" => {
                let key = ws
                    .llm_api_key_encrypted
                    .as_deref()
                    .map(|k| cipher.decrypt(k))
                    .transpose()?
                    .unwrap_or_default();
                tracing::info!(gen = gen_model, embed = embed_model, "LLM provider: OpenAI");
                (
                    Arc::new(OpenAiEmbeddingClient::with_model(&key, embed_model, 1536)),
                    Arc::new(OpenAiTextGenerationClient::with_model(&key, gen_model)),
                )
            }
            "anthropic" => {
                let key = ws
                    .llm_api_key_encrypted
                    .as_deref()
                    .map(|k| cipher.decrypt(k))
                    .transpose()?
                    .unwrap_or_default();
                let emb: Arc<dyn EmbeddingClient> = match std::env::var("EMBEDDING_API_KEY") {
                    Ok(k) if !k.is_empty() => {
                        Arc::new(OpenAiEmbeddingClient::with_model(&k, embed_model, 1536))
                    }
                    _ => Arc::new(StubEmbeddingClient::default()),
                };
                tracing::info!(gen = gen_model, "LLM provider: Anthropic");
                (
                    emb,
                    Arc::new(AnthropicTextGenerationClient::with_model(&key, gen_model)),
                )
            }
            "ollama" => {
                let base_url = ws
                    .llm_base_url
                    .as_deref()
                    .unwrap_or("http://localhost:11434");
                tracing::info!(
                    base_url,
                    gen = gen_model,
                    embed = embed_model,
                    "LLM provider: Ollama"
                );
                (
                    Arc::new(OllamaEmbeddingClient::new(base_url, embed_model)),
                    Arc::new(OllamaTextClient::new(base_url, gen_model)),
                )
            }
            _ => {
                tracing::info!("LLM provider: stub (test / unknown)");
                (
                    Arc::new(StubEmbeddingClient::default()),
                    Arc::new(StubTextGenerationClient),
                )
            }
        };

    Ok(pair)
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
