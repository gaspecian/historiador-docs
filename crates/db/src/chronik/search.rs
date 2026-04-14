//! Vector (HNSW) and full-text (Tantivy) search via Chronik REST API.
//!
//! The `published-pages` topic has both vector and full-text indexing
//! enabled. Chronik exposes search endpoints via its REST API.

use serde::{Deserialize, Serialize};

use super::ChronikClient;
use crate::vector_store::{ChunkEmbedding, ChunkRef, SearchFilters, VectorStoreError};

/// Request body for Chronik vector search.
#[derive(Debug, Serialize)]
struct VectorSearchRequest {
    topic: String,
    vector: Vec<f32>,
    k: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    filter: Option<SearchFilter>,
}

#[derive(Debug, Serialize)]
struct SearchFilter {
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    page_version_id: Option<String>,
}

/// Response from Chronik vector search.
#[derive(Debug, Deserialize)]
struct VectorSearchResponse {
    results: Vec<VectorSearchHit>,
}

#[derive(Debug, Deserialize)]
struct VectorSearchHit {
    id: String,
    score: f32,
    payload: serde_json::Value,
}

/// Request body for upserting embeddings into a Chronik topic.
#[derive(Debug, Serialize)]
struct UpsertRequest {
    topic: String,
    documents: Vec<UpsertDocument>,
}

#[derive(Debug, Serialize)]
struct UpsertDocument {
    key: String,
    payload: serde_json::Value,
    vector: Vec<f32>,
}

/// Request body for deleting documents from a Chronik topic.
#[derive(Debug, Serialize)]
struct DeleteRequest {
    topic: String,
    filter: DeleteFilter,
}

#[derive(Debug, Serialize)]
struct DeleteFilter {
    page_version_id: String,
}

impl ChronikClient {
    /// Upsert chunk embeddings into the `published-pages` topic.
    pub async fn upsert_chunks(
        &self,
        chunks: Vec<ChunkEmbedding>,
    ) -> Result<Vec<String>, VectorStoreError> {
        let documents: Vec<UpsertDocument> = chunks
            .iter()
            .map(|chunk| {
                let key = format!("{}:{}", chunk.page_version_id, chunk.section_index);
                UpsertDocument {
                    key: key.clone(),
                    payload: serde_json::json!({
                        "page_version_id": chunk.page_version_id,
                        "section_index": chunk.section_index,
                        "heading_path": chunk.heading_path,
                        "content": chunk.content,
                        "language": chunk.language,
                        "token_count": chunk.token_count,
                    }),
                    vector: chunk.embedding.clone(),
                }
            })
            .collect();

        let refs: Vec<String> = documents.iter().map(|d| d.key.clone()).collect();

        let url = format!("{}/api/v1/upsert", self.search_base_url);
        let resp = self
            .http
            .post(&url)
            .json(&UpsertRequest {
                topic: "published-pages".to_string(),
                documents,
            })
            .send()
            .await
            .map_err(|e| VectorStoreError::Internal(format!("chronik upsert failed: {e}")))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(VectorStoreError::Internal(format!(
                "chronik upsert error: {body}"
            )));
        }

        Ok(refs)
    }

    /// Search the `published-pages` topic using HNSW vector similarity.
    pub async fn vector_search(
        &self,
        query_embedding: &[f32],
        filters: SearchFilters,
        k: usize,
    ) -> Result<Vec<ChunkRef>, VectorStoreError> {
        let filter = if filters.language.is_some() || filters.page_version_id.is_some() {
            Some(SearchFilter {
                language: filters.language,
                page_version_id: filters.page_version_id,
            })
        } else {
            None
        };

        let url = format!("{}/api/v1/search", self.search_base_url);
        let resp = self
            .http
            .post(&url)
            .json(&VectorSearchRequest {
                topic: "published-pages".to_string(),
                vector: query_embedding.to_vec(),
                k,
                filter,
            })
            .send()
            .await
            .map_err(|e| VectorStoreError::Internal(format!("chronik search failed: {e}")))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(VectorStoreError::Internal(format!(
                "chronik search error: {body}"
            )));
        }

        let search_resp: VectorSearchResponse = resp
            .json()
            .await
            .map_err(|e| VectorStoreError::Internal(format!("chronik search parse: {e}")))?;

        let results = search_resp
            .results
            .into_iter()
            .map(|hit| {
                let payload = &hit.payload;
                ChunkRef {
                    vexfs_ref: hit.id,
                    score: hit.score,
                    content: payload
                        .get("content")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    heading_path: payload
                        .get("heading_path")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default(),
                    language: payload
                        .get("language")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    page_version_id: payload
                        .get("page_version_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                }
            })
            .collect();

        Ok(results)
    }

    /// Delete all documents matching a `page_version_id` from the
    /// `published-pages` topic.
    pub async fn delete_by_page_version(
        &self,
        page_version_id: &str,
    ) -> Result<u64, VectorStoreError> {
        let url = format!("{}/api/v1/delete", self.search_base_url);
        let resp = self
            .http
            .post(&url)
            .json(&DeleteRequest {
                topic: "published-pages".to_string(),
                filter: DeleteFilter {
                    page_version_id: page_version_id.to_string(),
                },
            })
            .send()
            .await
            .map_err(|e| VectorStoreError::Internal(format!("chronik delete failed: {e}")))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(VectorStoreError::Internal(format!(
                "chronik delete error: {body}"
            )));
        }

        // Chronik returns the count of deleted documents.
        #[derive(Deserialize)]
        struct DeleteResponse {
            deleted: u64,
        }

        let del_resp: DeleteResponse = resp
            .json()
            .await
            .map_err(|e| VectorStoreError::Internal(format!("chronik delete parse: {e}")))?;

        Ok(del_resp.deleted)
    }

    /// Check if Chronik search endpoint is healthy.
    pub async fn search_health(&self) -> Result<bool, VectorStoreError> {
        let url = format!("{}/health", self.search_base_url);
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(VectorStoreError::Http)?;
        Ok(resp.status().is_success())
    }
}
