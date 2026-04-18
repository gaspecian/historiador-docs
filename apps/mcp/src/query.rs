//! `POST /query` handler — thin Clean Architecture wrapper over
//! [`crate::application::SearchChunksUseCase`]. The handler only
//! translates DTOs, runs the timer for telemetry, and fires the
//! read-only proxied log to the API.
//!
//! The core logic is extracted into [`perform_query`] so the
//! JSON-RPC 2.0 `tools/call` handler in [`crate::jsonrpc`] can reuse
//! it without duplicating the telemetry path.

use std::sync::Arc;

use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};

use crate::application::SearchChunksCommand;
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
    pub total_results: usize,
    pub language_filter_applied: bool,
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
    perform_query(&state, body).await.map(Json).map_err(|e| {
        tracing::error!(error = %e, "search_chunks failed");
        StatusCode::INTERNAL_SERVER_ERROR
    })
}

/// Run a query against the vector store and fire the async telemetry
/// log. Shared by `POST /query` and the JSON-RPC `tools/call` dispatcher.
pub async fn perform_query(
    state: &Arc<McpState>,
    body: QueryRequest,
) -> Result<QueryResponse, anyhow::Error> {
    let start = std::time::Instant::now();
    let top_k = body.top_k.unwrap_or(5);
    let query_text_for_log = body.query.clone();

    let result = state
        .search_chunks
        .execute(SearchChunksCommand {
            query: body.query,
            language: body.language,
            top_k,
        })
        .await?;

    let chunks: Vec<ChunkResult> = result
        .chunks
        .into_iter()
        .map(|c| ChunkResult {
            content: c.content,
            heading_path: c.heading_path,
            page_title: c.page_title,
            collection_path: c.collection_path,
            score: c.score,
            language: c.language,
        })
        .collect();
    let total_results = chunks.len();
    let response_time_ms = start.elapsed().as_millis() as i32;

    // Fire-and-forget: log query to API for Chronik ingestion (ADR-003:
    // MCP stays read-only, logging is proxied through the API).
    let api_url = state.internal_api_url.clone();
    let workspace_id = state.workspace_id;
    let top_score = chunks.first().map(|c| c.score);
    let result_count = total_results as i32;
    tokio::spawn(async move {
        let _ = reqwest::Client::new()
            .post(format!("{api_url}/internal/mcp-log"))
            .json(&serde_json::json!({
                "query_text": query_text_for_log,
                "workspace_id": workspace_id,
                "result_count": result_count,
                "top_chunk_score": top_score,
                "response_time_ms": response_time_ms,
            }))
            .send()
            .await;
    });

    Ok(QueryResponse {
        chunks,
        total_results,
        language_filter_applied: result.language_filter_applied,
    })
}
