//! MCP query logging and analytics endpoints — thin Clean
//! Architecture wrappers over [`crate::application::admin`].

use std::sync::Arc;

use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::application::admin::LogMcpQueryCommand;
use crate::auth::extractor::AuthUser;
use crate::domain::port::query_analytics;
use crate::error::ApiError;
use crate::state::AppState;

// ---- MCP query log (internal endpoint) ----

#[derive(Debug, Deserialize)]
pub struct McpQueryEvent {
    pub query_text: String,
    pub workspace_id: Uuid,
    pub result_count: i32,
    // These fields are kept for wire-format compatibility with the
    // MCP server but are not yet carried through the domain event.
    pub top_chunk_score: Option<f32>,
    pub response_time_ms: i32,
}

pub async fn log_mcp_query(
    State(state): State<Arc<AppState>>,
    Json(event): Json<McpQueryEvent>,
) -> StatusCode {
    let _ = event.top_chunk_score;
    let _ = event.response_time_ms;

    // Fire-and-forget — failures log inside the adapter but never
    // bubble here.
    let _ = state
        .use_cases
        .log_mcp_query
        .execute(LogMcpQueryCommand {
            workspace_id: event.workspace_id,
            query_text: event.query_text,
            result_count: event.result_count,
        })
        .await;
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
    let stats = state
        .use_cases
        .get_mcp_analytics
        .execute(auth.as_actor(), params.days)
        .await?;
    Ok(Json(stats.into()))
}

// ---- response DTOs (moved from the old Chronik-coupled impl) ----

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

impl From<query_analytics::McpQueryStats> for McpAnalyticsResponse {
    fn from(s: query_analytics::McpQueryStats) -> Self {
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
