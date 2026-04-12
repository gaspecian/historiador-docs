//! `POST /query` handler — semantic search over chunked documentation.

use std::sync::Arc;

use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use historiador_db::{
    postgres::mcp_queries,
    vector_store::SearchFilters,
};

use crate::state::McpState;

// ---- DTOs ----

#[derive(Debug, Deserialize)]
pub struct QueryRequest {
    /// Natural-language query string.
    pub query: String,
    /// Optional BCP 47 language filter.
    pub language: Option<String>,
    /// Number of results to return (default 5, max 20).
    pub top_k: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct QueryResponse {
    pub chunks: Vec<ChunkResult>,
}

#[derive(Debug, Serialize)]
pub struct ChunkResult {
    pub content: String,
    pub heading_path: Vec<String>,
    pub page_title: String,
    pub collection_path: Vec<String>,
    pub score: f32,
    pub language: String,
}

// ---- handler ----

pub async fn handler(
    State(state): State<Arc<McpState>>,
    Json(body): Json<QueryRequest>,
) -> Result<Json<QueryResponse>, StatusCode> {
    let top_k = body.top_k.unwrap_or(5).min(20);

    // 1. Generate query embedding.
    let embeddings = state
        .embedding_client
        .embed(std::slice::from_ref(&body.query))
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to generate query embedding");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let query_vector = match embeddings.first() {
        Some(e) => &e.vector,
        None => return Ok(Json(QueryResponse { chunks: vec![] })),
    };

    // 2. Search the vector store.
    let filters = SearchFilters {
        language: body.language,
        ..Default::default()
    };

    let chunk_refs = state
        .vector_store
        .search(query_vector, filters, top_k)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "vector store search failed");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if chunk_refs.is_empty() {
        return Ok(Json(QueryResponse { chunks: vec![] }));
    }

    // 3. Collect page_version_ids for metadata enrichment.
    let pv_ids: Vec<Uuid> = chunk_refs
        .iter()
        .filter_map(|r| Uuid::parse_str(&r.page_version_id).ok())
        .collect();

    let meta_map = mcp_queries::enrich_chunk_results(&state.pool, &pv_ids)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "metadata enrichment failed");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // 4. Merge vector results with metadata.
    let chunks = chunk_refs
        .into_iter()
        .map(|cr| {
            let pv_id = Uuid::parse_str(&cr.page_version_id).ok();
            let meta = pv_id.and_then(|id| meta_map.get(&id));

            ChunkResult {
                content: cr.content,
                heading_path: cr.heading_path,
                page_title: meta.map(|m| m.page_title.clone()).unwrap_or_default(),
                collection_path: meta
                    .map(|m| m.collection_path.clone())
                    .unwrap_or_default(),
                score: cr.score,
                language: cr.language,
            }
        })
        .collect();

    Ok(Json(QueryResponse { chunks }))
}
