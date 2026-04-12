use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use sqlx::PgPool;

use crate::crypto::Cipher;
use crate::setup::llm_probe::LlmProbe;
use historiador_db::vector_store::VectorStore;
use historiador_llm::EmbeddingClient;

/// Shared application state. Every route handler receives this via
/// `State<Arc<AppState>>`.
pub struct AppState {
    pub pool: PgPool,
    pub git_sha: String,
    pub jwt_secret: Vec<u8>,
    pub cipher: Cipher,
    pub public_base_url: String,
    /// Cached setup-complete flag. Seeded at startup from the
    /// `installation` row and flipped to TRUE by the setup wizard
    /// after the DB transaction commits. Avoids a per-request DB
    /// round-trip on every gated route.
    pub setup_complete: AtomicBool,
    /// LLM probe — a trait object so the e2e test can swap in a
    /// stub that never hits the network.
    pub llm_probe: Arc<dyn LlmProbe>,
    /// Vector store for chunk embeddings (in-memory stub in Sprint 3).
    pub vector_store: Arc<dyn VectorStore>,
    /// Embedding client for generating text embeddings (stub in Sprint 3).
    pub embedding_client: Arc<dyn EmbeddingClient>,
}
