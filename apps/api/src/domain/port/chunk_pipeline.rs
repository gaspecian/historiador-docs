//! Chunk pipeline port — orchestrates markdown → chunks → embeddings
//! → vector store + Postgres `chunks` rows.

use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::error::ApplicationError;
use crate::domain::value::Language;

#[derive(Debug, Clone)]
pub struct ChunkPipelineInput {
    pub page_version_id: Uuid,
    pub language: Language,
    pub markdown: String,
}

#[async_trait]
pub trait ChunkPipeline: Send + Sync {
    /// Run the full pipeline for a single page version. Idempotent —
    /// callers may re-run when content changes; the adapter is
    /// responsible for deleting previous chunks.
    async fn run(&self, input: ChunkPipelineInput) -> Result<(), ApplicationError>;

    /// Delete all embeddings and Postgres chunk rows for a page
    /// version. Used when a page is unpublished or deleted.
    async fn clear(&self, page_version_id: Uuid) -> Result<(), ApplicationError>;
}
