//! Shared application state for the MCP server.

use std::sync::Arc;

use crate::application::SearchChunksUseCase;

/// MCP server state. Separate from the API's `AppState` because MCP
/// has different capabilities (read-only DB, bearer token auth, no
/// JWT). Handlers reach through `search_chunks` for business logic;
/// the remaining primitives cover wire-level concerns (bearer auth,
/// telemetry proxying to the API).
pub struct McpState {
    pub search_chunks: Arc<SearchChunksUseCase>,
    /// SHA-256 hash of the expected bearer token, computed at startup.
    pub bearer_token_hash: [u8; 32],
    /// Internal API URL for proxied query logging (ADR-003: MCP
    /// stays read-only; writes go through the API).
    pub internal_api_url: String,
    /// Workspace ID (v1 is single-workspace; loaded at boot).
    pub workspace_id: uuid::Uuid,
}
