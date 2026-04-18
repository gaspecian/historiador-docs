use std::sync::Arc;

use uuid::Uuid;

use crate::domain::error::{ApplicationError, DomainError};
use crate::domain::port::chunk_pipeline::{ChunkPipeline, ChunkPipelineInput};
use crate::domain::port::event_producer::{DomainEvent, EventProducer};
use crate::domain::port::page_repository::PageRepository;
use crate::domain::port::version_history_repository::{
    NewVersionSnapshot, VersionHistoryRepository,
};
use crate::domain::value::{Actor, PageStatus, Role};

pub struct PublishPageUseCase {
    pages: Arc<dyn PageRepository>,
    history: Arc<dyn VersionHistoryRepository>,
    pipeline: Arc<dyn ChunkPipeline>,
    events: Arc<dyn EventProducer>,
}

impl PublishPageUseCase {
    pub fn new(
        pages: Arc<dyn PageRepository>,
        history: Arc<dyn VersionHistoryRepository>,
        pipeline: Arc<dyn ChunkPipeline>,
        events: Arc<dyn EventProducer>,
    ) -> Self {
        Self {
            pages,
            history,
            pipeline,
            events,
        }
    }

    /// Synchronous part: flip status, snapshot, emit events. Returns
    /// the page id once those have committed. Chunking happens
    /// **asynchronously** via `spawn_chunk_pipeline` — presentation is
    /// responsible for spawning that, because `spawn` ties a future
    /// to the tokio runtime and should not happen deep in the use-case
    /// layer.
    pub async fn execute(&self, actor: Actor, page_id: Uuid) -> Result<Uuid, ApplicationError> {
        actor.require_role(Role::Author)?;

        let page = self
            .pages
            .find_by_id(page_id, actor.workspace_id)
            .await?
            .ok_or(DomainError::NotFound)?;

        self.pages
            .update_status(page.id, actor.workspace_id, PageStatus::Published)
            .await?;

        let versions = self.pages.find_versions(page.id).await?;

        for v in &versions {
            if let Err(e) = self
                .history
                .insert(NewVersionSnapshot {
                    page_id: v.page_id,
                    language: v.language.clone(),
                    title: v.title.clone(),
                    content_markdown: v.content_markdown.clone(),
                    is_published: true,
                    author_id: v.author_id,
                })
                .await
            {
                tracing::warn!(
                    page_id = %v.page_id,
                    language = %v.language.as_str(),
                    error = ?e,
                    "failed to snapshot version history on publish"
                );
            }
            self.events
                .publish(DomainEvent::PagePublished {
                    workspace_id: actor.workspace_id,
                    page_id: page.id,
                    page_version_id: v.id,
                    language: v.language.clone(),
                    title: v.title.clone(),
                })
                .await?;
        }

        Ok(page.id)
    }

    /// Run the chunk pipeline for every version of a page. Intended to
    /// be wrapped in `tokio::spawn` by the handler after `execute`
    /// returns. Fire-and-forget semantics — errors are logged, not
    /// propagated.
    pub async fn run_chunk_pipeline_for(&self, page_id: Uuid, workspace_id: Uuid) {
        let versions = match self.pages.find_versions(page_id).await {
            Ok(v) => v,
            Err(e) => {
                tracing::error!(%page_id, %workspace_id, error = ?e, "could not load versions for chunking");
                return;
            }
        };
        for version in versions {
            if let Err(e) = self
                .pipeline
                .run(ChunkPipelineInput {
                    page_version_id: version.id,
                    language: version.language,
                    markdown: version.content_markdown,
                })
                .await
            {
                tracing::error!(
                    page_version_id = %version.id,
                    error = ?e,
                    "async chunk pipeline failed"
                );
            }
        }
    }
}
