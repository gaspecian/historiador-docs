//! Chronik-Stream client — event streaming, vector search, full-text
//! search, and SQL analytics (ADR-007).
//!
//! Uses Chronik's REST API exclusively (no Kafka wire protocol) to
//! keep the dependency footprint minimal — only `reqwest` is needed.
//!
//! # Topic Architecture
//!
//! | Topic                  | Capabilities       | Purpose                            |
//! |------------------------|--------------------|------------------------------------|
//! | `published-pages`      | Vector + Full-text | MCP semantic search; dashboard FTS |
//! | `mcp-queries`          | SQL analytics      | Gap detection; usage reporting     |
//! | `editor-conversations` | Streaming only     | Durable conversation history       |
//! | `page-events`          | Streaming + SQL    | Audit log; webhook notifications   |

pub mod analytics;
pub mod producer;
pub mod search;

use reqwest::Client;

/// Unified Chronik-Stream client. Uses the REST API for all
/// operations: event production, vector/full-text search, and SQL
/// analytics queries.
#[derive(Clone)]
pub struct ChronikClient {
    /// Base URL for the Chronik REST API (e.g., `http://localhost:6092`).
    pub base_url: String,
    /// Base URL for Chronik search endpoints (vector + full-text).
    /// May differ from `base_url` in multi-port setups.
    pub search_base_url: String,
    /// Shared HTTP client for all REST API calls.
    pub http: Client,
}

/// Configuration for building a [`ChronikClient`].
pub struct ChronikConfig {
    /// Chronik REST API base URL (serves events + SQL analytics).
    pub base_url: String,
    /// Chronik search API base URL (vector + full-text).
    pub search_base_url: String,
}

impl ChronikClient {
    /// Create a new Chronik client from configuration.
    pub fn new(config: ChronikConfig) -> anyhow::Result<Self> {
        Ok(Self {
            base_url: config.base_url,
            search_base_url: config.search_base_url,
            http: Client::new(),
        })
    }
}
