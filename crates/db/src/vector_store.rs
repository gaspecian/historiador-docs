//! `VectorStore` trait + implementations.
//!
//! Sprint 3 ships an in-memory stub so the chunk pipeline runs
//! end-to-end without a real VexFS instance. The `HttpVexfsClient`
//! retains its health-check capability but returns `NotImplemented`
//! for search/upsert until VexFS's HTTP API is finalized.
//!
//! # Invariant (ADR-001)
//!
//! VexFS is the retrieval source of truth for chunk embeddings.
//! Postgres stores only `chunks.vexfs_ref` — an opaque pointer that
//! [`VectorStore`] implementations interpret. Never duplicate embeddings
//! into Postgres.

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum VectorStoreError {
    #[error("vexfs http error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("not implemented — VexFS wire integration pending")]
    NotImplemented,

    #[error("internal error: {0}")]
    Internal(String),
}

// ---- types for the expanded trait ----

/// A chunk with its embedding, ready to be upserted into the vector store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkEmbedding {
    pub page_version_id: String,
    pub section_index: i32,
    pub heading_path: Vec<String>,
    pub content: String,
    pub language: String,
    pub token_count: i32,
    pub embedding: Vec<f32>,
}

/// Opaque pointer to a chunk embedding stored in the vector store,
/// paired with a relevance score from a similarity search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkRef {
    pub vexfs_ref: String,
    pub score: f32,
    pub content: String,
    pub heading_path: Vec<String>,
    pub language: String,
    pub page_version_id: String,
}

/// Filters for similarity search.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchFilters {
    pub language: Option<String>,
    pub page_version_id: Option<String>,
}

// ---- trait ----

/// Abstraction over the vector store. Swappable so the API and MCP
/// server can be tested against an in-memory fake, and so a migration
/// to a different backend (e.g. Qdrant) never touches call sites.
#[async_trait]
pub trait VectorStore: Send + Sync {
    /// Returns `Ok(true)` if the underlying store responds to a health
    /// probe.
    async fn health(&self) -> Result<bool, VectorStoreError>;

    /// Upsert chunk embeddings. Returns the vexfs_ref identifiers
    /// assigned to each chunk (in the same order as the input).
    async fn upsert_chunks(
        &self,
        chunks: Vec<ChunkEmbedding>,
    ) -> Result<Vec<String>, VectorStoreError>;

    /// Top-k similarity search with metadata filters.
    async fn search(
        &self,
        query_embedding: &[f32],
        filters: SearchFilters,
        k: usize,
    ) -> Result<Vec<ChunkRef>, VectorStoreError>;

    /// Delete all chunk embeddings for a page_version.
    async fn delete_by_page_version(&self, page_version_id: &str) -> Result<u64, VectorStoreError>;
}

/// Returns `true` when operators have explicitly opted into the
/// in-memory vector store fallback by setting
/// `ALLOW_IN_MEMORY_VECTOR_STORE=true` (or `1`).
///
/// The default is `false`: if `CHRONIK_SQL_URL` is missing or Chronik
/// fails to initialize, callers should abort startup rather than
/// silently fall back to `InMemoryVectorStore`, whose data disappears on
/// every process restart and silently breaks durability-dependent
/// features like page version history.
///
/// Closes code review finding 4.4 / Sprint 10 item #3.
pub fn allow_in_memory_vector_store() -> bool {
    std::env::var("ALLOW_IN_MEMORY_VECTOR_STORE")
        .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
        .unwrap_or(false)
}

// ---- InMemoryVectorStore ----

/// In-memory vector store for Sprint 3. Data is lost on process
/// restart. Uses brute-force cosine similarity for search.
pub struct InMemoryVectorStore {
    store: RwLock<HashMap<String, ChunkEmbedding>>,
    counter: RwLock<u64>,
}

impl InMemoryVectorStore {
    pub fn new() -> Self {
        Self {
            store: RwLock::new(HashMap::new()),
            counter: RwLock::new(0),
        }
    }
}

impl Default for InMemoryVectorStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl VectorStore for InMemoryVectorStore {
    async fn health(&self) -> Result<bool, VectorStoreError> {
        Ok(true)
    }

    async fn upsert_chunks(
        &self,
        chunks: Vec<ChunkEmbedding>,
    ) -> Result<Vec<String>, VectorStoreError> {
        let mut store = self
            .store
            .write()
            .map_err(|e| VectorStoreError::Internal(format!("lock poisoned: {e}")))?;
        let mut counter = self
            .counter
            .write()
            .map_err(|e| VectorStoreError::Internal(format!("lock poisoned: {e}")))?;

        let mut refs = Vec::with_capacity(chunks.len());
        for chunk in chunks {
            *counter += 1;
            let vexfs_ref = format!("mem-{}", *counter);
            store.insert(vexfs_ref.clone(), chunk);
            refs.push(vexfs_ref);
        }
        Ok(refs)
    }

    async fn search(
        &self,
        query_embedding: &[f32],
        filters: SearchFilters,
        k: usize,
    ) -> Result<Vec<ChunkRef>, VectorStoreError> {
        let store = self
            .store
            .read()
            .map_err(|e| VectorStoreError::Internal(format!("lock poisoned: {e}")))?;

        let mut scored: Vec<(String, &ChunkEmbedding, f32)> = store
            .iter()
            .filter(|(_, chunk)| {
                if let Some(ref lang) = filters.language {
                    if &chunk.language != lang {
                        return false;
                    }
                }
                if let Some(ref pvid) = filters.page_version_id {
                    if &chunk.page_version_id != pvid {
                        return false;
                    }
                }
                true
            })
            .map(|(ref_id, chunk)| {
                let score = cosine_similarity(query_embedding, &chunk.embedding);
                (ref_id.clone(), chunk, score)
            })
            .collect();

        // Sort by score descending.
        scored.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(k);

        let results = scored
            .into_iter()
            .map(|(vexfs_ref, chunk, score)| ChunkRef {
                vexfs_ref,
                score,
                content: chunk.content.clone(),
                heading_path: chunk.heading_path.clone(),
                language: chunk.language.clone(),
                page_version_id: chunk.page_version_id.clone(),
            })
            .collect();

        Ok(results)
    }

    async fn delete_by_page_version(&self, page_version_id: &str) -> Result<u64, VectorStoreError> {
        let mut store = self
            .store
            .write()
            .map_err(|e| VectorStoreError::Internal(format!("lock poisoned: {e}")))?;
        let before = store.len();
        store.retain(|_, chunk| chunk.page_version_id != page_version_id);
        Ok((before - store.len()) as u64)
    }
}

/// Cosine similarity between two vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    dot / (norm_a * norm_b)
}

// ---- HttpVexfsClient (kept from Sprint 1) ----

/// HTTP client for VexFS's unified REST server.
///
/// Talks to VexFS via `reqwest`. The base URL typically looks like
/// `http://vexfs:7680` inside docker-compose. The upsert/search/delete
/// methods return `NotImplemented` until VexFS's HTTP API is finalized.
pub struct HttpVexfsClient {
    base_url: String,
    http: Client,
}

impl HttpVexfsClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            http: Client::new(),
        }
    }
}

#[async_trait]
impl VectorStore for HttpVexfsClient {
    async fn health(&self) -> Result<bool, VectorStoreError> {
        let url = format!("{}/api/v1/version", self.base_url);
        let resp = self.http.get(url).send().await?;
        Ok(resp.status().is_success())
    }

    async fn upsert_chunks(
        &self,
        _chunks: Vec<ChunkEmbedding>,
    ) -> Result<Vec<String>, VectorStoreError> {
        Err(VectorStoreError::NotImplemented)
    }

    async fn search(
        &self,
        _query_embedding: &[f32],
        _filters: SearchFilters,
        _k: usize,
    ) -> Result<Vec<ChunkRef>, VectorStoreError> {
        Err(VectorStoreError::NotImplemented)
    }

    async fn delete_by_page_version(
        &self,
        _page_version_id: &str,
    ) -> Result<u64, VectorStoreError> {
        Err(VectorStoreError::NotImplemented)
    }
}

// ---- ChronikVectorStore (Sprint 7, ADR-007) ----

/// Vector store backed by Chronik-Stream's HNSW index on the
/// `published-pages` topic. Replaces `InMemoryVectorStore` and
/// `HttpVexfsClient` as the production implementation.
pub struct ChronikVectorStore {
    client: crate::chronik::ChronikClient,
}

impl ChronikVectorStore {
    pub fn new(client: crate::chronik::ChronikClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl VectorStore for ChronikVectorStore {
    async fn health(&self) -> Result<bool, VectorStoreError> {
        self.client.search_health().await
    }

    async fn upsert_chunks(
        &self,
        chunks: Vec<ChunkEmbedding>,
    ) -> Result<Vec<String>, VectorStoreError> {
        self.client.upsert_chunks(chunks).await
    }

    async fn search(
        &self,
        query_embedding: &[f32],
        filters: SearchFilters,
        k: usize,
    ) -> Result<Vec<ChunkRef>, VectorStoreError> {
        self.client.vector_search(query_embedding, filters, k).await
    }

    async fn delete_by_page_version(&self, page_version_id: &str) -> Result<u64, VectorStoreError> {
        self.client.delete_by_page_version(page_version_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn in_memory_upsert_and_search() {
        let store = InMemoryVectorStore::new();

        let chunks = vec![
            ChunkEmbedding {
                page_version_id: "pv-1".into(),
                section_index: 0,
                heading_path: vec!["Intro".into()],
                content: "Hello world".into(),
                language: "en".into(),
                token_count: 2,
                embedding: vec![1.0, 0.0, 0.0],
            },
            ChunkEmbedding {
                page_version_id: "pv-1".into(),
                section_index: 1,
                heading_path: vec!["Details".into()],
                content: "More details".into(),
                language: "en".into(),
                token_count: 2,
                embedding: vec![0.0, 1.0, 0.0],
            },
        ];

        let refs = store.upsert_chunks(chunks).await.unwrap();
        assert_eq!(refs.len(), 2);

        // Search with a query close to the first chunk.
        let results = store
            .search(&[0.9, 0.1, 0.0], SearchFilters::default(), 10)
            .await
            .unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].content, "Hello world");
    }

    #[tokio::test]
    async fn in_memory_delete_by_page_version() {
        let store = InMemoryVectorStore::new();

        let chunks = vec![
            ChunkEmbedding {
                page_version_id: "pv-1".into(),
                section_index: 0,
                heading_path: vec![],
                content: "chunk a".into(),
                language: "en".into(),
                token_count: 2,
                embedding: vec![1.0],
            },
            ChunkEmbedding {
                page_version_id: "pv-2".into(),
                section_index: 0,
                heading_path: vec![],
                content: "chunk b".into(),
                language: "en".into(),
                token_count: 2,
                embedding: vec![1.0],
            },
        ];

        store.upsert_chunks(chunks).await.unwrap();
        let deleted = store.delete_by_page_version("pv-1").await.unwrap();
        assert_eq!(deleted, 1);

        // Only pv-2 chunk should remain.
        let results = store
            .search(&[1.0], SearchFilters::default(), 10)
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].page_version_id, "pv-2");
    }

    #[tokio::test]
    async fn in_memory_search_filters_by_language() {
        let store = InMemoryVectorStore::new();

        let chunks = vec![
            ChunkEmbedding {
                page_version_id: "pv-1".into(),
                section_index: 0,
                heading_path: vec![],
                content: "english".into(),
                language: "en".into(),
                token_count: 1,
                embedding: vec![1.0],
            },
            ChunkEmbedding {
                page_version_id: "pv-1".into(),
                section_index: 1,
                heading_path: vec![],
                content: "portuguese".into(),
                language: "pt-BR".into(),
                token_count: 1,
                embedding: vec![1.0],
            },
        ];

        store.upsert_chunks(chunks).await.unwrap();

        let results = store
            .search(
                &[1.0],
                SearchFilters {
                    language: Some("pt-BR".into()),
                    ..Default::default()
                },
                10,
            )
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content, "portuguese");
    }

    #[tokio::test]
    async fn in_memory_health_returns_true() {
        let store = InMemoryVectorStore::new();
        assert!(store.health().await.unwrap());
    }
}
