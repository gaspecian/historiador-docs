use std::net::SocketAddr;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use anyhow::Context;
use historiador_api::{
    app,
    crypto::Cipher,
    setup::llm_probe::HttpLlmProbe,
    state::AppState,
};
use historiador_db::{
    postgres::installation,
    vector_store::InMemoryVectorStore,
};
use historiador_llm::StubEmbeddingClient;
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
    tracing::info!(setup_complete = install.setup_complete, "installation loaded");

    let state = Arc::new(AppState {
        pool,
        git_sha,
        jwt_secret: jwt_secret.into_bytes(),
        cipher,
        public_base_url,
        setup_complete,
        llm_probe: Arc::new(HttpLlmProbe::default()),
        vector_store: Arc::new(InMemoryVectorStore::new()),
        embedding_client: Arc::new(StubEmbeddingClient::default()),
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
