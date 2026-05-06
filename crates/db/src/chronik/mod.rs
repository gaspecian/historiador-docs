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
///
/// `kafka_producer` is `None` when the client is constructed without a
/// Kafka broker (e.g. the MCP binary, which is read-only per ADR-003).
/// Callers that require write access (chunk pipeline, page-events) must
/// obtain the producer via `.kafka_producer.as_ref()`.
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
    /// `None` when constructed without a broker — read-only deployments
    /// such as the MCP server never need this.
    pub kafka_producer: Option<Arc<KafkaProducer>>,
}

pub struct ChronikConfig {
    pub base_url: String,
    pub search_base_url: String,
    /// Kafka broker `host:port`. `None` means "no Kafka" — the client
    /// will skip `KafkaProducer::connect` and set `kafka_producer = None`.
    /// Read-only services (e.g. MCP) should pass `None` here.
    pub kafka_broker: Option<String>,
}

impl ChronikClient {
    pub async fn new(config: ChronikConfig) -> anyhow::Result<Self> {
        let kafka_producer = match config.kafka_broker {
            Some(ref broker) => {
                let kafka = KafkaProducer::connect(broker)
                    .await
                    .map_err(|e| anyhow::anyhow!("connect to chronik kafka {broker}: {e}"))?;
                Some(Arc::new(kafka))
            }
            None => None,
        };
        Ok(Self {
            base_url: config.base_url,
            search_base_url: config.search_base_url,
            http: Client::new(),
            kafka_producer,
        })
    }
}
