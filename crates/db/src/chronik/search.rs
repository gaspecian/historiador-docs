//! Chronik REST search calls. Writes are not here — they go through
//! the Kafka producer in `crate::chronik::kafka_producer`.

use serde::{Deserialize, Serialize};

use super::ChronikClient;
use crate::chronik::producer::topics::PUBLISHED_PAGES;
use crate::vector_store::{ChunkRef, SearchFilters, VectorStoreError};

#[derive(Debug, Serialize)]
struct VectorSearchRequest {
    query: String,
    k: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    filters: Option<SearchFiltersWire>,
}

#[derive(Debug, Serialize)]
struct SearchFiltersWire {
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    page_version_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct VectorSearchResponse {
    results: Vec<VectorSearchHit>,
}

#[derive(Debug, Deserialize)]
struct VectorSearchHit {
    partition: i32,
    offset: i64,
    score: f32,
    #[serde(default)]
    text_preview: Option<String>,
}

impl ChronikClient {
    /// Semantic search via Chronik's `/_vector/<topic>/search` endpoint.
    /// Chronik embeds the query using the topic's configured model.
    pub async fn vector_search_text(
        &self,
        query: &str,
        filters: SearchFilters,
        k: usize,
    ) -> Result<Vec<ChunkRef>, VectorStoreError> {
        let url = format!(
            "{}/_vector/{}/search",
            self.search_base_url, PUBLISHED_PAGES
        );
        let body = VectorSearchRequest {
            query: query.to_string(),
            k,
            filters: if filters.language.is_some() || filters.page_version_id.is_some() {
                Some(SearchFiltersWire {
                    language: filters.language,
                    page_version_id: filters.page_version_id,
                })
            } else {
                None
            },
        };

        let resp = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| VectorStoreError::Internal(format!("chronik search request: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(VectorStoreError::Internal(format!(
                "chronik search HTTP {status}: {text}"
            )));
        }

        let parsed: VectorSearchResponse = resp
            .json()
            .await
            .map_err(|e| VectorStoreError::Internal(format!("chronik search parse: {e}")))?;

        Ok(parsed
            .results
            .into_iter()
            .map(|h| ChunkRef {
                partition: h.partition,
                offset: h.offset,
                score: h.score,
                text_preview: h.text_preview,
            })
            .collect())
    }

    /// Health check against Chronik's `/health` endpoint.
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
