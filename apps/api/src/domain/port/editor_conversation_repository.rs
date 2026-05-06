use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::entity::{EditorConversation, EditorConversationMessage};
use crate::domain::error::ApplicationError;
use crate::domain::value::Language;

#[async_trait]
pub trait EditorConversationRepository: Send + Sync {
    async fn find(
        &self,
        page_id: Uuid,
        language: &Language,
        user_id: Uuid,
    ) -> Result<Option<EditorConversation>, ApplicationError>;

    async fn upsert(
        &self,
        page_id: Uuid,
        language: &Language,
        user_id: Uuid,
        messages: &[EditorConversationMessage],
    ) -> Result<EditorConversation, ApplicationError>;
}
