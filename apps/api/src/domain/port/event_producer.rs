//! Event producer port — abstracts over Chronik-Stream (or any future
//! event broker). Use cases pass in a domain-shaped event struct; the
//! adapter chooses the topic and serializes the payload.

use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::error::ApplicationError;
use crate::domain::value::Language;

/// Events a use case may want to publish. New variants are added as
/// use cases emerge.
#[derive(Debug, Clone)]
pub enum DomainEvent {
    PagePublished {
        workspace_id: Uuid,
        page_id: Uuid,
        page_version_id: Uuid,
        language: Language,
        title: String,
    },
    PageUpdated {
        workspace_id: Uuid,
        page_id: Uuid,
        language: Language,
    },
    PageDrafted {
        workspace_id: Uuid,
        page_id: Uuid,
    },
    McpQueryLogged {
        workspace_id: Uuid,
        query_text: String,
        result_count: i64,
    },
    EditorDraftGenerated {
        workspace_id: Uuid,
        user_id: Uuid,
        prompt_tokens: Option<u32>,
    },
}

#[async_trait]
pub trait EventProducer: Send + Sync {
    /// Publish an event. Implementations that offer fire-and-forget
    /// semantics may swallow transient network errors internally; the
    /// return is `Ok(())` when the event has been accepted for delivery.
    async fn publish(&self, event: DomainEvent) -> Result<(), ApplicationError>;
}
