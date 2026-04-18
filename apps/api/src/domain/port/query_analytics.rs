//! Analytics query port — abstracts over Chronik's DataFusion SQL
//! endpoint. Input is the time window; output is a domain-shaped
//! stats struct with no persistence or HTTP coupling.

use async_trait::async_trait;

use crate::domain::error::ApplicationError;

#[derive(Debug, Clone)]
pub struct McpQueryStats {
    pub period_days: i32,
    pub total_queries: i64,
    pub queries_by_day: Vec<DayCount>,
    pub top_queries: Vec<QueryFrequency>,
    pub zero_result_queries: ZeroResultSummary,
}

#[derive(Debug, Clone)]
pub struct DayCount {
    pub date: String,
    pub count: i64,
}

#[derive(Debug, Clone)]
pub struct QueryFrequency {
    pub query_text: String,
    pub count: i64,
}

#[derive(Debug, Clone)]
pub struct ZeroResultSummary {
    pub count: i64,
    pub queries: Vec<ZeroResultQuery>,
}

#[derive(Debug, Clone)]
pub struct ZeroResultQuery {
    pub query_text: String,
    pub count: i64,
    pub last_seen: String,
}

#[async_trait]
pub trait QueryAnalytics: Send + Sync {
    async fn mcp_query_stats(&self, days: i32) -> Result<McpQueryStats, ApplicationError>;
}
