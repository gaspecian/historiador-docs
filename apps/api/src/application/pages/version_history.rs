use std::sync::Arc;

use uuid::Uuid;

use crate::domain::entity::{VersionHistoryEntry, VersionHistorySummary};
use crate::domain::error::{ApplicationError, DomainError};
use crate::domain::port::event_producer::{DomainEvent, EventProducer};
use crate::domain::port::page_repository::{PageRepository, UpsertPageVersion};
use crate::domain::port::version_history_repository::{
    NewVersionSnapshot, PageRequest, VersionHistoryRepository,
};
use crate::domain::value::{Actor, Language, PageStatus, Role};

use super::view::PageView;

// ---- list ----

pub struct ListVersionHistoryUseCase {
    pages: Arc<dyn PageRepository>,
    history: Arc<dyn VersionHistoryRepository>,
}

pub struct ListVersionHistoryCommand {
    pub page_id: Uuid,
    pub language: Language,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

pub struct VersionHistoryPage {
    pub summaries: Vec<VersionHistorySummary>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

impl ListVersionHistoryUseCase {
    pub fn new(pages: Arc<dyn PageRepository>, history: Arc<dyn VersionHistoryRepository>) -> Self {
        Self { pages, history }
    }

    pub async fn execute(
        &self,
        actor: Actor,
        cmd: ListVersionHistoryCommand,
    ) -> Result<VersionHistoryPage, ApplicationError> {
        actor.require_role(Role::Viewer)?;
        self.pages
            .find_by_id(cmd.page_id, actor.workspace_id)
            .await?
            .ok_or(DomainError::NotFound)?;

        let page = cmd.page.unwrap_or(1).max(1);
        let per_page = cmd.per_page.unwrap_or(20).clamp(1, 50);

        let (summaries, total) = self
            .history
            .list(cmd.page_id, &cmd.language, PageRequest { page, per_page })
            .await?;

        Ok(VersionHistoryPage {
            summaries,
            total,
            page,
            per_page,
        })
    }
}

// ---- get one ----

pub struct GetVersionHistoryItemUseCase {
    pages: Arc<dyn PageRepository>,
    history: Arc<dyn VersionHistoryRepository>,
}

impl GetVersionHistoryItemUseCase {
    pub fn new(pages: Arc<dyn PageRepository>, history: Arc<dyn VersionHistoryRepository>) -> Self {
        Self { pages, history }
    }

    pub async fn execute(
        &self,
        actor: Actor,
        page_id: Uuid,
        history_id: Uuid,
    ) -> Result<VersionHistoryEntry, ApplicationError> {
        actor.require_role(Role::Viewer)?;
        self.pages
            .find_by_id(page_id, actor.workspace_id)
            .await?
            .ok_or(DomainError::NotFound)?;

        let entry = self
            .history
            .find_by_id(history_id)
            .await?
            .ok_or(DomainError::NotFound)?;
        if entry.page_id != page_id {
            return Err(DomainError::NotFound.into());
        }
        Ok(entry)
    }
}

// ---- restore ----

pub struct RestoreVersionUseCase {
    pages: Arc<dyn PageRepository>,
    history: Arc<dyn VersionHistoryRepository>,
    events: Arc<dyn EventProducer>,
}

impl RestoreVersionUseCase {
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
        page_id: Uuid,
        history_id: Uuid,
    ) -> Result<PageView, ApplicationError> {
        actor.require_role(Role::Author)?;

        let page = self
            .pages
            .find_by_id(page_id, actor.workspace_id)
            .await?
            .ok_or(DomainError::NotFound)?;

        if matches!(page.status, PageStatus::Published) {
            return Err(DomainError::Validation(
                "page is published — revert to draft before restoring".into(),
            )
            .into());
        }

        let entry = self
            .history
            .find_by_id(history_id)
            .await?
            .ok_or(DomainError::NotFound)?;
        if entry.page_id != page_id {
            return Err(DomainError::NotFound.into());
        }

        self.pages
            .upsert_version(UpsertPageVersion {
                page_id: page.id,
                language: entry.language.clone(),
                title: entry.title.clone(),
                content_markdown: entry.content_markdown.clone(),
                author_id: actor.user_id,
                status: PageStatus::Draft,
            })
            .await?;

        // Record the restore in history itself.
        let _ = self
            .history
            .insert(NewVersionSnapshot {
                page_id: page.id,
                language: entry.language.clone(),
                title: entry.title.clone(),
                content_markdown: entry.content_markdown.clone(),
                is_published: false,
                author_id: Some(actor.user_id),
            })
            .await;

        self.events
            .publish(DomainEvent::PageUpdated {
                workspace_id: actor.workspace_id,
                page_id: page.id,
                language: entry.language,
            })
            .await?;

        let versions = self.pages.find_versions(page.id).await?;
        Ok(PageView { page, versions })
    }
}
