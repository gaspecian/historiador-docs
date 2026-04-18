//! `ChunkPipeline` adapter — composes the chunker, the embedding
//! client, and the vector store behind the domain port. The `pool`
//! handles the Postgres `chunks` metadata rows.
//!
//! This supersedes `crate::pages::pipeline::run_chunk_pipeline` once
//! the handler rewire lands. Kept in parallel during the refactor so
//! the existing fire-and-forget call site continues to work.

use std::sync::Arc;

use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use historiador_chunker::{chunk_markdown, ChunkConfig};
use historiador_db::postgres::chunks;
use historiador_db::vector_store::{ChunkEmbedding, VectorStore};
use historiador_llm::EmbeddingClient;

use crate::domain::error::ApplicationError;
use crate::domain::port::chunk_pipeline::{ChunkPipeline, ChunkPipelineInput};

pub struct DefaultChunkPipeline {
    pool: PgPool,
    vector_store: Arc<dyn VectorStore>,
    embedding_client: Arc<dyn EmbeddingClient>,
}

impl DefaultChunkPipeline {
    pub fn new(
        pool: PgPool,
        vector_store: Arc<dyn VectorStore>,
        embedding_client: Arc<dyn EmbeddingClient>,
    ) -> Self {
        Self {
            pool,
            vector_store,
            embedding_client,
        }
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

        let existing = chunks::find_by_page_version(&self.pool, page_version_id).await?;
        if !existing.is_empty() {
            self.vector_store
                .delete_by_page_version(&page_version_id.to_string())
                .await
                .map_err(|e| anyhow::anyhow!("vector store delete failed: {e}"))?;
            chunks::delete_by_page_version(&self.pool, page_version_id).await?;
        }

        let config = ChunkConfig::default();
        let raw_chunks = match chunk_markdown(&markdown, &config) {
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

        let texts: Vec<String> = raw_chunks.iter().map(|c| c.content.clone()).collect();
        let embeddings = self
            .embedding_client
            .embed(&texts)
            .await
            .map_err(|e| anyhow::anyhow!("embedding failed: {e}"))?;

        let chunk_embeddings: Vec<ChunkEmbedding> = raw_chunks
            .iter()
            .zip(embeddings.iter())
            .map(|(chunk, emb)| ChunkEmbedding {
                page_version_id: page_version_id.to_string(),
                section_index: chunk.section_index as i32,
                heading_path: chunk.heading_path.clone(),
                content: chunk.content.clone(),
                language: language.as_str().to_string(),
                token_count: chunk.token_count as i32,
                embedding: emb.vector.clone(),
            })
            .collect();

        let vexfs_refs = self
            .vector_store
            .upsert_chunks(chunk_embeddings)
            .await
            .map_err(|e| anyhow::anyhow!("vector store upsert failed: {e}"))?;

        let new_chunks: Vec<chunks::NewChunk> = raw_chunks
            .iter()
            .zip(vexfs_refs.iter())
            .map(|(chunk, vexfs_ref)| chunks::NewChunk {
                page_version_id,
                heading_path: chunk.heading_path.clone(),
                section_index: chunk.section_index as i32,
                token_count: chunk.token_count as i32,
                oversized: chunk.oversized,
                language: language.as_str().to_string(),
                vexfs_ref: vexfs_ref.clone(),
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
        self.vector_store
            .delete_by_page_version(&page_version_id.to_string())
            .await
            .map_err(|e| anyhow::anyhow!("vector store delete failed: {e}"))?;
        chunks::delete_by_page_version(&self.pool, page_version_id).await?;
        Ok(())
    }
}
