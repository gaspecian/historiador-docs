//! Async chunk → embed → upsert pipeline.
//!
//! Triggered by `POST /pages/:id/publish` via `tokio::spawn`. Errors
//! are logged but never propagate to the HTTP response (fire-and-forget).

use historiador_chunker::{chunk_markdown, ChunkConfig};
use historiador_db::postgres::{chunks, page_versions::PageVersion};
use historiador_db::vector_store::{ChunkEmbedding, VectorStore};
use historiador_llm::EmbeddingClient;
use sqlx::PgPool;

/// Run the full chunk pipeline for a single page version:
/// 1. Delete old chunks (Postgres + VectorStore)
/// 2. Chunk the markdown
/// 3. Generate embeddings
/// 4. Upsert to VectorStore
/// 5. Insert chunk metadata to Postgres
pub async fn run_chunk_pipeline(
    pool: &PgPool,
    vector_store: &dyn VectorStore,
    embedding_client: &dyn EmbeddingClient,
    version: &PageVersion,
) -> anyhow::Result<()> {
    // 1. Delete old chunks.
    let old_chunks = chunks::find_by_page_version(pool, version.id).await?;
    if !old_chunks.is_empty() {
        vector_store
            .delete_by_page_version(&version.id.to_string())
            .await
            .map_err(|e| anyhow::anyhow!("vexfs delete failed: {e}"))?;
        chunks::delete_by_page_version(pool, version.id).await?;
    }

    // 2. Chunk the markdown.
    let config = ChunkConfig::default();
    let raw_chunks = match chunk_markdown(&version.content_markdown, &config) {
        Ok(c) => c,
        Err(historiador_chunker::ChunkError::EmptyInput) => {
            tracing::warn!(page_version_id = %version.id, "empty content, skipping chunk pipeline");
            return Ok(());
        }
    };

    if raw_chunks.is_empty() {
        tracing::warn!(page_version_id = %version.id, "chunker produced no chunks");
        return Ok(());
    }

    // 3. Generate embeddings.
    let texts: Vec<String> = raw_chunks.iter().map(|c| c.content.clone()).collect();
    let embeddings = embedding_client
        .embed(&texts)
        .await
        .map_err(|e| anyhow::anyhow!("embedding failed: {e}"))?;

    // 4. Build ChunkEmbedding structs for VectorStore.
    let chunk_embeddings: Vec<ChunkEmbedding> = raw_chunks
        .iter()
        .zip(embeddings.iter())
        .map(|(chunk, emb)| ChunkEmbedding {
            page_version_id: version.id.to_string(),
            section_index: chunk.section_index as i32,
            heading_path: chunk.heading_path.clone(),
            content: chunk.content.clone(),
            language: version.language.clone(),
            token_count: chunk.token_count as i32,
            embedding: emb.vector.clone(),
        })
        .collect();

    // 5. Upsert to VectorStore, get back vexfs_refs.
    let vexfs_refs = vector_store
        .upsert_chunks(chunk_embeddings)
        .await
        .map_err(|e| anyhow::anyhow!("vexfs upsert failed: {e}"))?;

    // 6. Insert chunk metadata into Postgres.
    let new_chunks: Vec<chunks::NewChunk> = raw_chunks
        .iter()
        .zip(vexfs_refs.iter())
        .map(|(chunk, vexfs_ref)| chunks::NewChunk {
            page_version_id: version.id,
            heading_path: chunk.heading_path.clone(),
            section_index: chunk.section_index as i32,
            token_count: chunk.token_count as i32,
            oversized: chunk.oversized,
            language: version.language.clone(),
            vexfs_ref: vexfs_ref.clone(),
        })
        .collect();

    chunks::insert_batch(pool, &new_chunks).await?;

    tracing::info!(
        page_version_id = %version.id,
        chunk_count = new_chunks.len(),
        "chunk pipeline complete"
    );

    Ok(())
}
