//! `QueryAnalytics` adapter — wraps
//! [`historiador_db::chronik::ChronikClient::mcp_query_stats`] and
//! translates its DTOs into the domain-level stats struct.

use async_trait::async_trait;

use historiador_db::chronik::analytics as db_stats;
use historiador_db::chronik::ChronikClient;

use crate::domain::error::{ApplicationError, DomainError};
use crate::domain::port::query_analytics::{
    DayCount, McpQueryStats, QueryAnalytics, QueryFrequency, ZeroResultQuery, ZeroResultSummary,
};

pub struct ChronikQueryAnalytics {
    client: Option<ChronikClient>,
}

impl ChronikQueryAnalytics {
    pub fn new(client: Option<ChronikClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl QueryAnalytics for ChronikQueryAnalytics {
    async fn mcp_query_stats(&self, days: i32) -> Result<McpQueryStats, ApplicationError> {
        let client = self.client.as_ref().ok_or_else(|| {
            DomainError::Validation("analytics unavailable — Chronik not configured".into())
        })?;
        let raw = client.mcp_query_stats(days).await?;
        Ok(map_stats(raw))
    }
}

fn map_stats(s: db_stats::McpQueryStats) -> McpQueryStats {
    McpQueryStats {
        period_days: s.period_days,
        total_queries: s.total_queries,
        queries_by_day: s
            .queries_by_day
            .into_iter()
            .map(|d| DayCount {
                date: d.date,
                count: d.count,
            })
            .collect(),
        top_queries: s
            .top_queries
            .into_iter()
            .map(|q| QueryFrequency {
                query_text: q.query_text,
                count: q.count,
            })
            .collect(),
        zero_result_queries: ZeroResultSummary {
            count: s.zero_result_queries.count,
            queries: s
                .zero_result_queries
                .queries
                .into_iter()
                .map(|q| ZeroResultQuery {
                    query_text: q.query_text,
                    count: q.count,
                    last_seen: q.last_seen,
                })
                .collect(),
        },
    }
}
