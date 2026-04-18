use std::sync::Arc;

use uuid::Uuid;

use crate::domain::error::{ApplicationError, DomainError};
use crate::domain::port::event_producer::{DomainEvent, EventProducer};
use crate::domain::port::page_repository::{PageRepository, UpsertPageVersion};
use crate::domain::port::version_history_repository::{
    NewVersionSnapshot, VersionHistoryRepository,
};
use crate::domain::value::{Actor, Language, PageStatus, Role};

use super::view::PageView;

/// Debounce window (seconds) for implicit version-history snapshots
/// on save. Publish always snapshots regardless.
const SNAPSHOT_DEBOUNCE_SECONDS: i32 = 30;

pub struct UpdatePageCommand {
    pub page_id: Uuid,
    pub language: Option<Language>,
    pub title: Option<String>,
    pub content_markdown: Option<String>,
}

pub struct UpdatePageUseCase {
    pages: Arc<dyn PageRepository>,
    history: Arc<dyn VersionHistoryRepository>,
    events: Arc<dyn EventProducer>,
}

impl UpdatePageUseCase {
    pub fn new(
        pages: Arc<dyn PageRepository>,
        history: Arc<dyn VersionHistoryRepository>,
        events: Arc<dyn EventProducer>,
    ) -> Self {
        Self {
            pages,
            history,
            events,
        }
    }

    pub async fn execute(
        &self,
        actor: Actor,
        cmd: UpdatePageCommand,
    ) -> Result<PageView, ApplicationError> {
        actor.require_role(Role::Author)?;

        let page = self
            .pages
            .find_by_id(cmd.page_id, actor.workspace_id)
            .await?
            .ok_or(DomainError::NotFound)?;

        if matches!(page.status, PageStatus::Published) {
            return Err(DomainError::Validation(
                "page is published — revert to draft before editing".into(),
            )
            .into());
        }

        // Default to "en" if no language specified — matches current
        // handler behavior. Callers should pass a language explicitly.
        let language = cmd.language.unwrap_or_else(|| Language::from_trusted("en"));

        let existing = self.pages.find_version(page.id, &language).await?;
        let title = cmd
            .title
            .or_else(|| existing.as_ref().map(|v| v.title.clone()))
            .unwrap_or_else(|| "Untitled".to_string());
        let content = cmd
            .content_markdown
            .or_else(|| existing.as_ref().map(|v| v.content_markdown.clone()))
            .unwrap_or_default();

        self.pages
            .upsert_version(UpsertPageVersion {
                page_id: page.id,
                language: language.clone(),
                title: title.clone(),
                content_markdown: content.clone(),
                author_id: actor.user_id,
                status: PageStatus::Draft,
            })
            .await?;

        // Debounced snapshot — skip if one was taken within the window.
        let has_recent = self
            .history
            .has_recent_snapshot(page.id, &language, SNAPSHOT_DEBOUNCE_SECONDS)
            .await
            .unwrap_or(true);
        if !has_recent {
            if let Err(e) = self
                .history
                .insert(NewVersionSnapshot {
                    page_id: page.id,
                    language: language.clone(),
                    title: title.clone(),
                    content_markdown: content.clone(),
                    is_published: false,
                    author_id: Some(actor.user_id),
                })
                .await
            {
                tracing::warn!(page_id = %page.id, error = ?e, "failed to snapshot on save");
            }
        }

        self.events
            .publish(DomainEvent::PageUpdated {
                workspace_id: actor.workspace_id,
                page_id: page.id,
                language,
            })
            .await?;

        let versions = self.pages.find_versions(page.id).await?;
        Ok(PageView { page, versions })
    }
}
