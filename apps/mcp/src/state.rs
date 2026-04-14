//! Shared application state for the MCP server.

use std::sync::Arc;

use sqlx::PgPool;

use historiador_db::vector_store::VectorStore;
use historiador_llm::EmbeddingClient;

/// MCP server state. Separate from the API's `AppState` because MCP
/// has different capabilities (read-only DB, bearer token auth, no JWT).
pub struct McpState {
    pub pool: PgPool,
    pub vector_store: Arc<dyn VectorStore>,
    pub embedding_client: Arc<dyn EmbeddingClient>,
    /// SHA-256 hash of the expected bearer token, computed at startup.
    pub bearer_token_hash: [u8; 32],
    /// Internal API URL for proxied logging (ADR-003: MCP stays read-only).
    pub internal_api_url: String,
    /// Workspace ID (v1 is single-workspace; loaded at boot).
    pub workspace_id: uuid::Uuid,
}
