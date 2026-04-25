//! `VectorStore` trait + implementations.
//!
//! Production backend is Chronik-Stream (ADR-007). Writes go through
//! Kafka (Chronik has no HTTP write path); reads go through Chronik's
//! REST `/_vector/<topic>/search` (text query, Chronik embeds).
//! Hits are addressed by `(partition, offset)` because Chronik does
//! not assign user-controllable doc ids.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::RwLock;
use thiserror::Error;

use crate::chronik::kafka_producer::ProducedRecord;

#[derive(Debug, Error)]
pub enum VectorStoreError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("kafka error: {0}")]
    Kafka(String),

    #[error("internal error: {0}")]
    Internal(String),
}

/// A chunk's payload as published to Chronik. Chronik embeds the
/// `content` field per the topic's `vector.field=$.content` config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkPayload {
    pub page_version_id: String,
    pub section_index: i32,
    pub heading_path: Vec<String>,
    pub content: String,
    pub language: String,
    pub token_count: i32,
}

/// A reference to a chunk located in Chronik, paired with the
/// similarity score from a search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkRef {
    pub partition: i32,
    pub offset: i64,
    pub score: f32,
    /// `text_preview` from Chronik (may be truncated). The
    /// authoritative content lives in the Postgres `chunks` row joined
    /// during MCP enrichment.
    pub text_preview: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchFilters {
    pub language: Option<String>,
    pub page_version_id: Option<String>,
}

#[async_trait]
pub trait VectorStore: Send + Sync {
    async fn health(&self) -> Result<bool, VectorStoreError>;

    /// Produce chunk records to Chronik. Returns the
    /// `(partition, offset)` for each chunk in input order.
    async fn produce_chunks(
        &self,
        chunks: Vec<ChunkPayload>,
    ) -> Result<Vec<ProducedRecord>, VectorStoreError>;

    /// Top-k semantic search on the topic. The query text is embedded
    /// by Chronik using the topic's configured embedding model.
    async fn search(
        &self,
        query: &str,
        filters: SearchFilters,
        k: usize,
    ) -> Result<Vec<ChunkRef>, VectorStoreError>;
}

pub fn allow_in_memory_vector_store() -> bool {
    std::env::var("ALLOW_IN_MEMORY_VECTOR_STORE")
        .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
        .unwrap_or(false)
}

// ---- InMemoryVectorStore ----

/// In-memory store for unit tests. Substring-matches the query
/// against chunk content and returns synthetic `(partition, offset)`
/// pairs. Not a semantic search — only useful to exercise the
/// upstream code paths.
pub struct InMemoryVectorStore {
    store: RwLock<Vec<(ProducedRecord, ChunkPayload)>>,
    next_offset: RwLock<i64>,
}

impl InMemoryVectorStore {
    pub fn new() -> Self {
        Self {
            store: RwLock::new(Vec::new()),
            next_offset: RwLock::new(0),
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

    async fn produce_chunks(
        &self,
        chunks: Vec<ChunkPayload>,
    ) -> Result<Vec<ProducedRecord>, VectorStoreError> {
        let mut store = self
            .store
            .write()
            .map_err(|e| VectorStoreError::Internal(format!("lock poisoned: {e}")))?;
        let mut next = self
            .next_offset
            .write()
            .map_err(|e| VectorStoreError::Internal(format!("lock poisoned: {e}")))?;

        let mut records = Vec::with_capacity(chunks.len());
        for chunk in chunks {
            let rec = ProducedRecord {
                partition: 0,
                offset: *next,
            };
            *next += 1;
            store.push((rec, chunk));
            records.push(rec);
        }
        Ok(records)
    }

    async fn search(
        &self,
        query: &str,
        filters: SearchFilters,
        k: usize,
    ) -> Result<Vec<ChunkRef>, VectorStoreError> {
        let store = self
            .store
            .read()
            .map_err(|e| VectorStoreError::Internal(format!("lock poisoned: {e}")))?;
        let q = query.to_lowercase();
        let mut hits: Vec<ChunkRef> = store
            .iter()
            .filter(|(_, c)| {
                if let Some(ref lang) = filters.language {
                    if &c.language != lang {
                        return false;
                    }
                }
                if let Some(ref pv) = filters.page_version_id {
                    if &c.page_version_id != pv {
                        return false;
                    }
                }
                true
            })
            .filter_map(|(rec, c)| {
                let lc = c.content.to_lowercase();
                if lc.contains(&q) {
                    Some(ChunkRef {
                        partition: rec.partition,
                        offset: rec.offset,
                        score: 1.0,
                        text_preview: Some(c.content.chars().take(200).collect()),
                    })
                } else {
                    None
                }
            })
            .collect();
        hits.truncate(k);
        Ok(hits)
    }
}

// ---- ChronikVectorStore ----

/// Vector store backed by Chronik-Stream. Writes go through Kafka
/// (`ChronikClient::kafka_producer`), reads go through the REST
/// `/_vector/<topic>/search` endpoint.
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

    async fn produce_chunks(
        &self,
        chunks: Vec<ChunkPayload>,
    ) -> Result<Vec<ProducedRecord>, VectorStoreError> {
        use crate::chronik::producer::topics::PUBLISHED_PAGES;

        let producer = self
            .client
            .kafka_producer
            .as_ref()
            .ok_or_else(|| VectorStoreError::Kafka("kafka producer not configured".to_string()))?;

        let mut out = Vec::with_capacity(chunks.len());
        for chunk in chunks {
            let key = format!("{}:{}", chunk.page_version_id, chunk.section_index);
            let payload = serde_json::json!({
                "page_version_id": chunk.page_version_id,
                "section_index": chunk.section_index,
                "heading_path": chunk.heading_path,
                "content": chunk.content,
                "language": chunk.language,
                "token_count": chunk.token_count,
            });
            let rec = producer
                .produce(PUBLISHED_PAGES, &key, &payload)
                .await
                .map_err(|e| VectorStoreError::Kafka(e.to_string()))?;
            out.push(rec);
        }
        Ok(out)
    }

    async fn search(
        &self,
        query: &str,
        filters: SearchFilters,
        k: usize,
    ) -> Result<Vec<ChunkRef>, VectorStoreError> {
        self.client.vector_search_text(query, filters, k).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn in_memory_substring_search() {
        let store = InMemoryVectorStore::new();
        store
            .produce_chunks(vec![
                ChunkPayload {
                    page_version_id: "pv-1".into(),
                    section_index: 0,
                    heading_path: vec!["Intro".into()],
                    content: "Hello world".into(),
                    language: "en".into(),
                    token_count: 2,
                },
                ChunkPayload {
                    page_version_id: "pv-1".into(),
                    section_index: 1,
                    heading_path: vec!["Details".into()],
                    content: "More details".into(),
                    language: "en".into(),
                    token_count: 2,
                },
            ])
            .await
            .unwrap();

        let hits = store.search("hello", SearchFilters::default(), 10).await.unwrap();
        assert_eq!(hits.len(), 1);
    }

    #[tokio::test]
    async fn in_memory_health_returns_true() {
        let store = InMemoryVectorStore::new();
        assert!(store.health().await.unwrap());
    }
}
