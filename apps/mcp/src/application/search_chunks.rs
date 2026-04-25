//! Semantic-search use case: query string → Chronik vector search →
//! Postgres metadata enrichment → `SearchChunksResult`.
//!
//! The `EmbeddingClient` is no longer involved on the read side —
//! Chronik embeds the query using the topic's configured model so
//! both write and read sides agree on the embedding space.

use std::sync::Arc;

use historiador_db::vector_store::{SearchFilters, VectorStore};

use super::port::ChunkMetadataReader;
use super::McpError;

#[derive(Debug, Clone)]
pub struct SearchChunksCommand {
    pub query: String,
    pub language: Option<String>,
    pub top_k: usize,
}

#[derive(Debug, Clone)]
pub struct SearchChunkResult {
    pub content: String,
    pub heading_path: Vec<String>,
    pub page_title: String,
    pub collection_path: Vec<String>,
    pub score: f32,
    pub language: String,
}

#[derive(Debug, Clone)]
pub struct SearchChunksResult {
    pub chunks: Vec<SearchChunkResult>,
    pub language_filter_applied: bool,
}

pub struct SearchChunksUseCase {
    vector_store: Arc<dyn VectorStore>,
    metadata: Arc<dyn ChunkMetadataReader>,
}

impl SearchChunksUseCase {
    pub fn new(vector_store: Arc<dyn VectorStore>, metadata: Arc<dyn ChunkMetadataReader>) -> Self {
        Self { vector_store, metadata }
    }

    pub async fn execute(&self, cmd: SearchChunksCommand) -> Result<SearchChunksResult, McpError> {
        let language_filter_applied = cmd.language.is_some();
        let top_k = cmd.top_k.clamp(1, 20);

        let filters = SearchFilters {
            language: cmd.language,
            ..Default::default()
        };

        let hits = self
            .vector_store
            .search(&cmd.query, filters, top_k)
            .await
            .map_err(|e| anyhow::anyhow!("chronik search failed: {e}"))?;

        if hits.is_empty() {
            return Ok(SearchChunksResult { chunks: vec![], language_filter_applied });
        }

        let refs: Vec<(i32, i64)> = hits.iter().map(|h| (h.partition, h.offset)).collect();
        let meta_map = self.metadata.enrich_many(&refs).await?;

        let chunks = hits
            .into_iter()
            .filter_map(|h| {
                let key = (h.partition, h.offset);
                let m = meta_map.get(&key)?;
                Some(SearchChunkResult {
                    content: m.content_markdown.clone(),
                    heading_path: m.heading_path.clone(),
                    page_title: m.page_title.clone(),
                    collection_path: m.collection_path.clone(),
                    score: h.score,
                    language: m.language.clone(),
                })
            })
            .collect();

        Ok(SearchChunksResult { chunks, language_filter_applied })
    }
}
