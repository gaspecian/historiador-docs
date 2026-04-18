//! Read-only ports exposed to the MCP application layer. Only
//! read-capable traits are referenced here; write repository methods
//! are neither imported nor constructible from this crate, so the
//! ADR-003 read-only invariant holds at compile time.

use std::collections::HashMap;

use async_trait::async_trait;
use uuid::Uuid;

use super::McpError;

/// Enriched metadata for a chunk, used to build MCP query responses.
/// Full shape preserved even though some fields are unused today — the
/// port contract is set by what the DB query returns, and consumers
/// may grow to need `page_id` / `collection_id` for deep-linking.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct ChunkMetadata {
    pub page_title: String,
    pub language: String,
    pub page_id: Uuid,
    pub collection_id: Option<Uuid>,
    pub collection_path: Vec<String>,
}

#[async_trait]
pub trait ChunkMetadataReader: Send + Sync {
    /// Given a batch of `page_version_id`s, return enriched metadata
    /// for each. Missing IDs are simply absent from the returned map.
    async fn enrich_many(
        &self,
        page_version_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, ChunkMetadata>, McpError>;
}
