//! DataFusion SQL analytics client for querying Chronik topics.
//!
//! Chronik exposes a REST API on port 6092 that accepts SQL queries
//! and returns JSON results backed by DataFusion against Arrow/Parquet
//! columnar storage.

use serde::{Deserialize, Serialize};

use super::ChronikClient;

/// A row returned by the DataFusion SQL REST API.
pub type SqlRow = serde_json::Value;

/// Response from the Chronik SQL REST API.
#[derive(Debug, Deserialize)]
pub struct SqlResponse {
    pub rows: Vec<SqlRow>,
    pub row_count: usize,
}

/// MCP query analytics aggregated by the API.
#[derive(Debug, Serialize)]
pub struct McpQueryStats {
    pub period_days: i32,
    pub total_queries: i64,
    pub queries_by_day: Vec<DayCount>,
    pub top_queries: Vec<QueryFrequency>,
    pub zero_result_queries: ZeroResultSummary,
}

#[derive(Debug, Serialize)]
pub struct DayCount {
    pub date: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct QueryFrequency {
    pub query_text: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct ZeroResultSummary {
    pub count: i64,
    pub queries: Vec<ZeroResultQuery>,
}

#[derive(Debug, Serialize)]
pub struct ZeroResultQuery {
    pub query_text: String,
    pub count: i64,
    pub last_seen: String,
}

impl ChronikClient {
    /// Execute a SQL query against the Chronik DataFusion REST API.
    pub async fn query_sql(&self, sql: &str) -> anyhow::Result<SqlResponse> {
        let url = format!("{}/api/v1/sql", self.base_url);

        let resp = self
            .http
            .post(&url)
            .json(&serde_json::json!({ "sql": sql }))
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("chronik sql request failed: {e}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("chronik sql error ({status}): {body}");
        }

        let result: SqlResponse = resp
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("chronik sql response parse failed: {e}"))?;

        Ok(result)
    }

    /// Fetch aggregated MCP query analytics for a given time window.
    pub async fn mcp_query_stats(&self, days: i32) -> anyhow::Result<McpQueryStats> {
        // Total queries in the window.
        let total_resp = self
            .query_sql(&format!(
                "SELECT COUNT(*) as total FROM \"mcp-queries\" \
                 WHERE timestamp >= now() - INTERVAL '{days} days'"
            ))
            .await?;

        let total_queries = total_resp
            .rows
            .first()
            .and_then(|r| r.get("total"))
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        // Queries grouped by day.
        let daily_resp = self
            .query_sql(&format!(
                "SELECT CAST(DATE_TRUNC('day', timestamp) AS VARCHAR) as day, \
                        COUNT(*) as count \
                 FROM \"mcp-queries\" \
                 WHERE timestamp >= now() - INTERVAL '{days} days' \
                 GROUP BY day ORDER BY day"
            ))
            .await?;

        let queries_by_day: Vec<DayCount> = daily_resp
            .rows
            .iter()
            .map(|r| DayCount {
                date: r
                    .get("day")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                count: r.get("count").and_then(|v| v.as_i64()).unwrap_or(0),
            })
            .collect();

        // Top 10 query texts by frequency.
        let top_resp = self
            .query_sql(&format!(
                "SELECT query_text, COUNT(*) as freq \
                 FROM \"mcp-queries\" \
                 WHERE timestamp >= now() - INTERVAL '{days} days' \
                 GROUP BY query_text ORDER BY freq DESC LIMIT 10"
            ))
            .await?;

        let top_queries: Vec<QueryFrequency> = top_resp
            .rows
            .iter()
            .map(|r| QueryFrequency {
                query_text: r
                    .get("query_text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                count: r.get("freq").and_then(|v| v.as_i64()).unwrap_or(0),
            })
            .collect();

        // Zero-result queries.
        let zero_resp = self
            .query_sql(&format!(
                "SELECT query_text, COUNT(*) as count, \
                        CAST(MAX(timestamp) AS VARCHAR) as last_seen \
                 FROM \"mcp-queries\" \
                 WHERE result_count = 0 \
                   AND timestamp >= now() - INTERVAL '{days} days' \
                 GROUP BY query_text ORDER BY count DESC LIMIT 20"
            ))
            .await?;

        let zero_queries: Vec<ZeroResultQuery> = zero_resp
            .rows
            .iter()
            .map(|r| ZeroResultQuery {
                query_text: r
                    .get("query_text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                count: r.get("count").and_then(|v| v.as_i64()).unwrap_or(0),
                last_seen: r
                    .get("last_seen")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            })
            .collect();

        let zero_total: i64 = zero_queries.iter().map(|q| q.count).sum();

        Ok(McpQueryStats {
            period_days: days,
            total_queries,
            queries_by_day,
            top_queries,
            zero_result_queries: ZeroResultSummary {
                count: zero_total,
                queries: zero_queries,
            },
        })
    }
}
