//! Event producer for writing to Chronik topics via REST API.

use super::ChronikClient;

/// Well-known Chronik topic names.
pub mod topics {
    pub const PUBLISHED_PAGES: &str = "published-pages";
    pub const MCP_QUERIES: &str = "mcp-queries";
    pub const EDITOR_CONVERSATIONS: &str = "editor-conversations";
    pub const PAGE_EVENTS: &str = "page-events";
}

impl ChronikClient {
    /// Produce a JSON event to a Chronik topic via the Kafka wire
    /// protocol. `key` is used for partitioning.
    pub async fn produce_event(
        &self,
        topic: &str,
        key: &str,
        payload: &serde_json::Value,
    ) -> anyhow::Result<()> {
        let producer = self
            .kafka_producer
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("kafka producer not configured (read-only client)"))?;
        producer
            .produce(topic, key, payload)
            .await
            .map_err(|e| anyhow::anyhow!("chronik produce ({topic}): {e}"))?;
        Ok(())
    }

    /// Fire-and-forget event production. Logs errors but never blocks
    /// the caller. Use for non-critical telemetry (MCP query logging,
    /// page events).
    pub fn produce_event_fire_and_forget(
        &self,
        topic: &'static str,
        key: String,
        payload: serde_json::Value,
    ) {
        let client = self.clone();
        tokio::spawn(async move {
            if let Err(e) = client.produce_event(topic, &key, &payload).await {
                tracing::warn!(
                    %topic,
                    error = %e,
                    "fire-and-forget event production failed"
                );
            }
        });
    }
}
