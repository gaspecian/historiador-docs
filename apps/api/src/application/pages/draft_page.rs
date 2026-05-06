use std::sync::Arc;

use uuid::Uuid;

use crate::domain::error::{ApplicationError, DomainError};
use crate::domain::port::chunk_pipeline::ChunkPipeline;
use crate::domain::port::event_producer::{DomainEvent, EventProducer};
use crate::domain::port::page_repository::PageRepository;
use crate::domain::value::{Actor, PageStatus, Role};

use super::view::PageView;

pub struct DraftPageUseCase {
    pages: Arc<dyn PageRepository>,
    pipeline: Arc<dyn ChunkPipeline>,
    events: Arc<dyn EventProducer>,
}

impl DraftPageUseCase {
    pub fn new(
        pages: Arc<dyn PageRepository>,
        pipeline: Arc<dyn ChunkPipeline>,
        events: Arc<dyn EventProducer>,
    ) -> Self {
        Self {
            pages,
            pipeline,
            events,
        }
    }

    pub async fn execute(&self, actor: Actor, page_id: Uuid) -> Result<PageView, ApplicationError> {
        actor.require_role(Role::Author)?;

        let page = self
            .pages
            .find_by_id(page_id, actor.workspace_id)
            .await?
            .ok_or(DomainError::NotFound)?;

        self.pages
            .update_status(page.id, actor.workspace_id, PageStatus::Draft)
            .await?;

        let versions = self.pages.find_versions(page.id).await?;
        for v in &versions {
            if let Err(e) = self.pipeline.clear(v.id).await {
                tracing::warn!(page_version_id = %v.id, error = ?e, "clear chunks failed");
            }
        }

        self.events
            .publish(DomainEvent::PageDrafted {
                workspace_id: actor.workspace_id,
                page_id: page.id,
            })
            .await?;

        Ok(PageView { page, versions })
    }
}
