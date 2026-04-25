//! Chronik-Stream client — Kafka writes (port 9092), REST reads
//! (search + SQL on port 6092). ADR-007.
//!
//! Chronik 2.4.1 has no HTTP write endpoint. Producers must use the
//! Kafka wire protocol; the REST surface is read-only (search,
//! analytics SQL, health). See `kafka_producer` for the write path
//! and `search` / `analytics` for the read paths.
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
pub mod kafka_producer;
pub mod producer;
pub mod search;

use std::sync::Arc;

use reqwest::Client;

use crate::chronik::kafka_producer::KafkaProducer;

/// Unified Chronik-Stream client. REST handles search + SQL; Kafka
/// handles writes (Chronik 2.4.1 has no HTTP write path).
#[derive(Clone)]
pub struct ChronikClient {
    /// REST base URL (search + SQL). E.g. `http://localhost:6092`.
    pub base_url: String,
    /// Search base URL — same host in single-port deploys, different
    /// host in split deploys.
    pub search_base_url: String,
    /// HTTP client for all REST API calls.
    pub http: Client,
    /// Kafka producer for writes (publish, page-events, etc.).
    pub kafka_producer: Arc<KafkaProducer>,
}

pub struct ChronikConfig {
    pub base_url: String,
    pub search_base_url: String,
    /// Kafka broker `host:port`.
    pub kafka_broker: String,
}

impl ChronikClient {
    pub async fn new(config: ChronikConfig) -> anyhow::Result<Self> {
        let kafka = KafkaProducer::connect(&config.kafka_broker)
            .await
            .map_err(|e| anyhow::anyhow!("connect to chronik kafka {}: {e}", config.kafka_broker))?;
        Ok(Self {
            base_url: config.base_url,
            search_base_url: config.search_base_url,
            http: Client::new(),
            kafka_producer: Arc::new(kafka),
        })
    }
}
