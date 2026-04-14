//! MCP query logging and analytics endpoints.
//!
//! - `POST /internal/mcp-log` — receives fire-and-forget query events
//!   from the MCP server and forwards them to Chronik's `mcp-queries`
//!   topic. Internal-only (no JWT auth, protected by network topology).
//!
//! - `GET /admin/analytics/mcp-queries` — admin-only dashboard data
//!   powered by Chronik's DataFusion SQL REST API.

use std::sync::Arc;

use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use historiador_db::chronik::{analytics::McpQueryStats, producer::topics};

use crate::auth::{extractor::AuthUser, rbac::require_role};
use crate::error::ApiError;
use crate::state::AppState;
use historiador_db::postgres::users::Role;

// ---- MCP query log (internal endpoint) ----

#[derive(Debug, Deserialize)]
pub struct McpQueryEvent {
    pub query_text: String,
    pub workspace_id: Uuid,
    pub result_count: i32,
    pub top_chunk_score: Option<f32>,
    pub response_time_ms: i32,
}

/// Receives MCP query events via API-proxied logging (ADR-003: MCP
/// stays read-only). Produces the event to the Chronik `mcp-queries`
/// topic. No auth — internal-only, protected by network topology.
pub async fn log_mcp_query(
    State(state): State<Arc<AppState>>,
    Json(event): Json<McpQueryEvent>,
) -> StatusCode {
    let Some(ref chronik) = state.chronik else {
        // Chronik not configured — silently drop.
        return StatusCode::ACCEPTED;
    };

    let payload = serde_json::json!({
        "query_text": event.query_text,
        "workspace_id": event.workspace_id,
        "result_count": event.result_count,
        "top_chunk_score": event.top_chunk_score,
        "response_time_ms": event.response_time_ms,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });

    chronik.produce_event_fire_and_forget(
        topics::MCP_QUERIES,
        event.workspace_id.to_string(),
        payload,
    );

    StatusCode::ACCEPTED
}

// ---- Analytics dashboard (admin endpoint) ----

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct AnalyticsQuery {
    /// Number of days to look back (default 7, max 30).
    pub days: Option<i32>,
}

#[utoipa::path(
    get,
    path = "/admin/analytics/mcp-queries",
    params(AnalyticsQuery),
    responses(
        (status = 200, description = "MCP query analytics", body = McpAnalyticsResponse),
        (status = 401, description = "unauthorized"),
        (status = 403, description = "forbidden"),
        (status = 503, description = "Chronik not configured"),
    ),
    security(("bearer" = [])),
    tag = "admin"
)]
pub async fn get_mcp_analytics(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    axum::extract::Query(params): axum::extract::Query<AnalyticsQuery>,
) -> Result<Json<McpAnalyticsResponse>, ApiError> {
    require_role(&auth, Role::Admin)?;

    let Some(ref chronik) = state.chronik else {
        return Err(ApiError::Validation(
            "analytics unavailable — Chronik not configured".into(),
        ));
    };

    let days = params.days.unwrap_or(7).clamp(1, 30);

    let stats = chronik
        .mcp_query_stats(days)
        .await
        .map_err(ApiError::Internal)?;

    Ok(Json(McpAnalyticsResponse::from(stats)))
}

/// Response DTO for the analytics endpoint. Mirrors the Chronik
/// `McpQueryStats` but with `utoipa::ToSchema` for OpenAPI generation.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct McpAnalyticsResponse {
    pub period_days: i32,
    pub total_queries: i64,
    pub queries_by_day: Vec<DayCountDto>,
    pub top_queries: Vec<QueryFrequencyDto>,
    pub zero_result_queries: ZeroResultSummaryDto,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct DayCountDto {
    pub date: String,
    pub count: i64,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct QueryFrequencyDto {
    pub query_text: String,
    pub count: i64,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ZeroResultSummaryDto {
    pub count: i64,
    pub queries: Vec<ZeroResultQueryDto>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ZeroResultQueryDto {
    pub query_text: String,
    pub count: i64,
    pub last_seen: String,
}

impl From<McpQueryStats> for McpAnalyticsResponse {
    fn from(s: McpQueryStats) -> Self {
        Self {
            period_days: s.period_days,
            total_queries: s.total_queries,
            queries_by_day: s
                .queries_by_day
                .into_iter()
                .map(|d| DayCountDto {
                    date: d.date,
                    count: d.count,
                })
                .collect(),
            top_queries: s
                .top_queries
                .into_iter()
                .map(|q| QueryFrequencyDto {
                    query_text: q.query_text,
                    count: q.count,
                })
                .collect(),
            zero_result_queries: ZeroResultSummaryDto {
                count: s.zero_result_queries.count,
                queries: s
                    .zero_result_queries
                    .queries
                    .into_iter()
                    .map(|q| ZeroResultQueryDto {
                        query_text: q.query_text,
                        count: q.count,
                        last_seen: q.last_seen,
                    })
                    .collect(),
            },
        }
    }
}
