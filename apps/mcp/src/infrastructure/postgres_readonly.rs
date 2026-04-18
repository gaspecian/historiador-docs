//! Postgres adapter for `ChunkMetadataReader`. Wraps the existing
//! recursive-CTE walk in `historiador_db::postgres::mcp_queries`.
//!
//! This module only imports the read-only query helper and cannot
//! reach any write path — the ADR-003 MCP invariant holds by
//! construction.

use std::collections::HashMap;

use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use historiador_db::postgres::mcp_queries;

use crate::application::port::{ChunkMetadata, ChunkMetadataReader};
use crate::application::McpError;

pub struct PostgresChunkMetadataReader {
    pool: PgPool,
}

impl PostgresChunkMetadataReader {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ChunkMetadataReader for PostgresChunkMetadataReader {
    async fn enrich_many(
        &self,
        page_version_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, ChunkMetadata>, McpError> {
        let raw = mcp_queries::enrich_chunk_results(&self.pool, page_version_ids).await?;
        Ok(raw
            .into_iter()
            .map(|(id, m)| {
                (
                    id,
                    ChunkMetadata {
                        page_title: m.page_title,
                        language: m.language,
                        page_id: m.page_id,
                        collection_id: m.collection_id,
                        collection_path: m.collection_path,
                    },
                )
            })
            .collect())
    }
}
