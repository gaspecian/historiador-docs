//! Editor-conversation persistence use cases (Sprint 10).
//!
//! Load and save the chat transcript for a `(page, language, user)`
//! triple so refreshing the editor or navigating away does not lose the
//! in-progress dialogue. The caller is the web client's debounced
//! auto-save hook; the infrastructure is a single Postgres row
//! upserted in-place.

use std::sync::Arc;

use uuid::Uuid;

use crate::domain::entity::{EditorConversation, EditorConversationMessage};
use crate::domain::error::{ApplicationError, DomainError};
use crate::domain::port::editor_conversation_repository::EditorConversationRepository;
use crate::domain::port::page_repository::PageRepository;
use crate::domain::value::{Actor, Language, Role};

pub struct LoadEditorConversationUseCase {
    pages: Arc<dyn PageRepository>,
    conversations: Arc<dyn EditorConversationRepository>,
}

impl LoadEditorConversationUseCase {
    pub fn new(
        pages: Arc<dyn PageRepository>,
        conversations: Arc<dyn EditorConversationRepository>,
    ) -> Self {
        Self {
            pages,
            conversations,
        }
    }

    pub async fn execute(
        &self,
        actor: Actor,
        page_id: Uuid,
        language: Language,
    ) -> Result<Option<EditorConversation>, ApplicationError> {
        actor.require_role(Role::Author)?;
        assert_page_belongs_to_workspace(&*self.pages, page_id, actor.workspace_id).await?;
        self.conversations
            .find(page_id, &language, actor.user_id)
            .await
    }
}

pub struct SaveEditorConversationCommand {
    pub page_id: Uuid,
    pub language: Language,
    pub messages: Vec<EditorConversationMessage>,
}

pub struct SaveEditorConversationUseCase {
    pages: Arc<dyn PageRepository>,
    conversations: Arc<dyn EditorConversationRepository>,
}

impl SaveEditorConversationUseCase {
    pub fn new(
        pages: Arc<dyn PageRepository>,
        conversations: Arc<dyn EditorConversationRepository>,
    ) -> Self {
        Self {
            pages,
            conversations,
        }
    }

    pub async fn execute(
        &self,
        actor: Actor,
        command: SaveEditorConversationCommand,
    ) -> Result<EditorConversation, ApplicationError> {
        actor.require_role(Role::Author)?;
        assert_page_belongs_to_workspace(&*self.pages, command.page_id, actor.workspace_id).await?;

        // Cap at 500 messages so a misbehaving client cannot grow the
        // row unbounded. Real conversations top out well below this.
        let messages = if command.messages.len() > 500 {
            command.messages[command.messages.len() - 500..].to_vec()
        } else {
            command.messages
        };

        self.conversations
            .upsert(command.page_id, &command.language, actor.user_id, &messages)
            .await
    }
}

async fn assert_page_belongs_to_workspace(
    pages: &dyn PageRepository,
    page_id: Uuid,
    workspace_id: Uuid,
) -> Result<(), ApplicationError> {
    pages
        .find_by_id(page_id, workspace_id)
        .await?
        .ok_or(DomainError::NotFound)?;
    Ok(())
}
