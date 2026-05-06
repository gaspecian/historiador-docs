use std::sync::Arc;

use uuid::Uuid;

use crate::domain::error::{ApplicationError, DomainError};
use crate::domain::port::event_producer::{DomainEvent, EventProducer};
use crate::domain::port::page_repository::{NewPage, PageRepository, UpsertPageVersion};
use crate::domain::value::{Actor, Language, PageStatus, Role, Slug};
use crate::util::slugify;

use super::view::PageView;

pub struct CreatePageCommand {
    pub collection_id: Option<Uuid>,
    pub title: String,
    pub content_markdown: String,
    pub language: Language,
}

pub struct CreatePageUseCase {
    pages: Arc<dyn PageRepository>,
    events: Arc<dyn EventProducer>,
}

impl CreatePageUseCase {
    pub fn new(pages: Arc<dyn PageRepository>, events: Arc<dyn EventProducer>) -> Self {
        Self { pages, events }
    }

    pub async fn execute(
        &self,
        actor: Actor,
        cmd: CreatePageCommand,
    ) -> Result<PageView, ApplicationError> {
        actor.require_role(Role::Author)?;

        let slug = Slug::parse(slugify(&cmd.title))?;

        let page = self
            .pages
            .insert(NewPage {
                workspace_id: actor.workspace_id,
                collection_id: cmd.collection_id,
                slug,
                created_by: actor.user_id,
            })
            .await
            .map_err(map_unique_violation)?;

        let version = self
            .pages
            .upsert_version(UpsertPageVersion {
                page_id: page.id,
                language: cmd.language.clone(),
                title: cmd.title,
                content_markdown: cmd.content_markdown,
                author_id: actor.user_id,
                status: PageStatus::Draft,
            })
            .await?;

        self.events
            .publish(DomainEvent::PageUpdated {
                workspace_id: actor.workspace_id,
                page_id: page.id,
                language: cmd.language,
            })
            .await?;

        Ok(PageView {
            page,
            versions: vec![version],
        })
    }
}

fn map_unique_violation(err: ApplicationError) -> ApplicationError {
    if let ApplicationError::Infrastructure(ref any_err) = err {
        let msg = any_err.to_string();
        if msg.contains("duplicate key") || msg.contains("unique constraint") {
            return ApplicationError::Domain(DomainError::Conflict(
                "page with this slug already exists in the collection".into(),
            ));
        }
    }
    err
}
