//! Chunk pipeline — splits the published markdown into chunks,
//! produces them to Chronik via Kafka, and stores the back-pointer
//! `(partition, offset)` in Postgres.
//!
//! Republish is a full reindex: existing `chunks` rows for the page
//! version are deleted, then all new chunks are produced and stored.
//! Chronik records produced before the reindex become orphans (no
//! Postgres row); MCP enrichment skips them.

use std::sync::Arc;

use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use historiador_chunker::{chunk_markdown, ChunkConfig};
use historiador_db::postgres::chunks;
use historiador_db::vector_store::{ChunkPayload, VectorStore};

use crate::domain::error::ApplicationError;
use crate::domain::port::chunk_pipeline::{ChunkPipeline, ChunkPipelineInput};

pub struct DefaultChunkPipeline {
    pool: PgPool,
    vector_store: Arc<dyn VectorStore>,
}

impl DefaultChunkPipeline {
    pub fn new(pool: PgPool, vector_store: Arc<dyn VectorStore>) -> Self {
        Self { pool, vector_store }
    }
}

#[async_trait]
impl ChunkPipeline for DefaultChunkPipeline {
    async fn run(&self, input: ChunkPipelineInput) -> Result<(), ApplicationError> {
        let ChunkPipelineInput {
            page_version_id,
            language,
            markdown,
        } = input;

        // Full reindex: delete existing chunks rows for this page
        // version. Chronik records become orphans and are filtered out
        // by MCP enrichment (no matching Postgres row).
        chunks::delete_by_page_version(&self.pool, page_version_id).await?;

        let raw_chunks = match chunk_markdown(&markdown, &ChunkConfig::default()) {
            Ok(c) => c,
            Err(historiador_chunker::ChunkError::EmptyInput) => {
                tracing::warn!(%page_version_id, "empty content, skipping chunk pipeline");
                return Ok(());
            }
        };
        if raw_chunks.is_empty() {
            tracing::warn!(%page_version_id, "chunker produced no chunks");
            return Ok(());
        }

        let payloads: Vec<ChunkPayload> = raw_chunks
            .iter()
            .map(|c| ChunkPayload {
                page_version_id: page_version_id.to_string(),
                section_index: c.section_index as i32,
                heading_path: c.heading_path.clone(),
                content: c.content.clone(),
                language: language.as_str().to_string(),
                token_count: c.token_count as i32,
            })
            .collect();

        let produced = self
            .vector_store
            .produce_chunks(payloads)
            .await
            .map_err(|e| anyhow::anyhow!("chronik produce failed: {e}"))?;

        let new_chunks: Vec<chunks::NewChunk> = raw_chunks
            .iter()
            .zip(produced.iter())
            .map(|(c, rec)| chunks::NewChunk {
                page_version_id,
                heading_path: c.heading_path.clone(),
                section_index: c.section_index as i32,
                token_count: c.token_count as i32,
                oversized: c.oversized,
                language: language.as_str().to_string(),
                chronik_partition: rec.partition,
                chronik_offset: rec.offset,
            })
            .collect();

        chunks::insert_batch(&self.pool, &new_chunks).await?;

        tracing::info!(
            %page_version_id,
            chunk_count = new_chunks.len(),
            "chunk pipeline complete"
        );
        Ok(())
    }

    async fn clear(&self, page_version_id: Uuid) -> Result<(), ApplicationError> {
        // No equivalent in Chronik — orphans are tolerated by design.
        chunks::delete_by_page_version(&self.pool, page_version_id).await?;
        Ok(())
    }
}
