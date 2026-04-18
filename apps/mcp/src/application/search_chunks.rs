//! Semantic-search use case: query string → embedding → vector search
//! → metadata enrichment → `SearchChunksResult`.

use std::sync::Arc;

use uuid::Uuid;

use historiador_db::vector_store::{SearchFilters, VectorStore};
use historiador_llm::EmbeddingClient;

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
    embedding_client: Arc<dyn EmbeddingClient>,
    vector_store: Arc<dyn VectorStore>,
    metadata: Arc<dyn ChunkMetadataReader>,
}

impl SearchChunksUseCase {
    pub fn new(
        embedding_client: Arc<dyn EmbeddingClient>,
        vector_store: Arc<dyn VectorStore>,
        metadata: Arc<dyn ChunkMetadataReader>,
    ) -> Self {
        Self {
            embedding_client,
            vector_store,
            metadata,
        }
    }

    pub async fn execute(
        &self,
        cmd: SearchChunksCommand,
    ) -> Result<SearchChunksResult, McpError> {
        let language_filter_applied = cmd.language.is_some();
        let top_k = cmd.top_k.clamp(1, 20);

        // 1. Embed the query.
        let embeddings = self
            .embedding_client
            .embed(std::slice::from_ref(&cmd.query))
            .await
            .map_err(|e| anyhow::anyhow!("embedding failed: {e}"))?;

        let query_vector = match embeddings.first() {
            Some(e) => e.vector.clone(),
            None => {
                return Ok(SearchChunksResult {
                    chunks: vec![],
                    language_filter_applied,
                })
            }
        };

        // 2. Vector search.
        let filters = SearchFilters {
            language: cmd.language,
            ..Default::default()
        };
        let chunk_refs = self
            .vector_store
            .search(&query_vector, filters, top_k)
            .await
            .map_err(|e| anyhow::anyhow!("vector store search failed: {e}"))?;

        if chunk_refs.is_empty() {
            return Ok(SearchChunksResult {
                chunks: vec![],
                language_filter_applied,
            });
        }

        // 3. Enrich with metadata.
        let pv_ids: Vec<Uuid> = chunk_refs
            .iter()
            .filter_map(|r| Uuid::parse_str(&r.page_version_id).ok())
            .collect();
        let meta_map = self.metadata.enrich_many(&pv_ids).await?;

        // 4. Merge.
        let chunks = chunk_refs
            .into_iter()
            .map(|cr| {
                let pv_id = Uuid::parse_str(&cr.page_version_id).ok();
                let meta = pv_id.and_then(|id| meta_map.get(&id));
                SearchChunkResult {
                    content: cr.content,
                    heading_path: cr.heading_path,
                    page_title: meta.map(|m| m.page_title.clone()).unwrap_or_default(),
                    collection_path: meta.map(|m| m.collection_path.clone()).unwrap_or_default(),
                    score: cr.score,
                    language: cr.language,
                }
            })
            .collect();

        Ok(SearchChunksResult {
            chunks,
            language_filter_applied,
        })
    }
}
