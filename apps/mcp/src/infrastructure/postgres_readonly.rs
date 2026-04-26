//! Postgres adapter for `ChunkMetadataReader`. Wraps the existing
//! recursive-CTE walk in `historiador_db::postgres::mcp_queries`.
//!
//! This module only imports the read-only query helper and cannot
//! reach any write path — the ADR-003 MCP invariant holds by
//! construction.

use std::collections::HashMap;

use async_trait::async_trait;
use sqlx::PgPool;

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
        refs: &[(i32, i64)],
    ) -> Result<HashMap<(i32, i64), ChunkMetadata>, McpError> {
        let raw = mcp_queries::enrich_chunk_results(&self.pool, refs).await?;
        Ok(raw
            .into_iter()
            .map(|((p, o), m)| {
                (
                    (p, o),
                    ChunkMetadata {
                        page_id: m.page_id,
                        page_version_id: m.page_version_id,
                        collection_id: m.collection_id,
                        page_title: m.page_title,
                        language: m.language,
                        collection_path: m.collection_path,
                        heading_path: m.heading_path,
                        content_markdown: m.content_markdown,
                    },
                )
            })
            .collect())
    }
}
