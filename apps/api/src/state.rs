use sqlx::PgPool;

/// Shared application state. Every route handler receives this via
/// `State<Arc<AppState>>`. Sprint 2 will grow this with the LLM client,
/// the VectorStore implementation, and the workspace cache.
pub struct AppState {
    pub pool: PgPool,
    pub git_sha: String,
}
