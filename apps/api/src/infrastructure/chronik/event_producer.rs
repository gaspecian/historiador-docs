//! Chronik-backed `EventProducer` adapter.
//!
//! Uses fire-and-forget semantics (matches the existing handlers):
//! failures are logged at `warn` but never propagate to callers. This
//! is correct for telemetry but wrong for anything the domain
//! actually needs to succeed — if a use case ever needs durable
//! delivery, add a separate synchronous port.

use async_trait::async_trait;
use serde_json::json;

use historiador_db::chronik::{producer::topics, ChronikClient};

use crate::domain::error::ApplicationError;
use crate::domain::port::event_producer::{DomainEvent, EventProducer};

pub struct ChronikEventProducer {
    client: Option<ChronikClient>,
}

impl ChronikEventProducer {
    /// Construct a producer. When `client` is `None` every publish
    /// becomes a no-op — useful for local dev without Chronik-Stream
    /// running, and for tests.
    pub fn new(client: Option<ChronikClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl EventProducer for ChronikEventProducer {
    async fn publish(&self, event: DomainEvent) -> Result<(), ApplicationError> {
        let Some(client) = self.client.as_ref() else {
            return Ok(());
        };
        let (topic, key, payload) = encode(&event);
        client.produce_event_fire_and_forget(topic, key, payload);
        Ok(())
    }
}

fn encode(event: &DomainEvent) -> (&'static str, String, serde_json::Value) {
    match event {
        DomainEvent::PagePublished {
            workspace_id,
            page_id,
            page_version_id,
            language,
            title,
        } => (
            topics::PUBLISHED_PAGES,
            page_id.to_string(),
            json!({
                "type": "page.published",
                "workspace_id": workspace_id,
                "page_id": page_id,
                "page_version_id": page_version_id,
                "language": language.as_str(),
                "title": title,
            }),
        ),
        DomainEvent::PageUpdated {
            workspace_id,
            page_id,
            language,
        } => (
            topics::PAGE_EVENTS,
            page_id.to_string(),
            json!({
                "type": "page.updated",
                "workspace_id": workspace_id,
                "page_id": page_id,
                "language": language.as_str(),
            }),
        ),
        DomainEvent::PageDrafted {
            workspace_id,
            page_id,
        } => (
            topics::PAGE_EVENTS,
            page_id.to_string(),
            json!({
                "type": "page.drafted",
                "workspace_id": workspace_id,
                "page_id": page_id,
            }),
        ),
        DomainEvent::McpQueryLogged {
            workspace_id,
            query_text,
            result_count,
        } => (
            topics::MCP_QUERIES,
            workspace_id.to_string(),
            json!({
                "workspace_id": workspace_id,
                "query_text": query_text,
                "result_count": result_count,
            }),
        ),
        DomainEvent::EditorDraftGenerated {
            workspace_id,
            user_id,
            prompt_tokens,
        } => (
            topics::EDITOR_CONVERSATIONS,
            user_id.to_string(),
            json!({
                "type": "editor.draft_generated",
                "workspace_id": workspace_id,
                "user_id": user_id,
                "prompt_tokens": prompt_tokens,
            }),
        ),
    }
}
