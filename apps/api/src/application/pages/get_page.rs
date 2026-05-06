use std::sync::Arc;

use uuid::Uuid;

use crate::domain::error::{ApplicationError, DomainError};
use crate::domain::port::page_repository::PageRepository;
use crate::domain::value::{Actor, Role};

use super::view::PageView;

pub struct GetPageUseCase {
    pages: Arc<dyn PageRepository>,
}

impl GetPageUseCase {
    pub fn new(pages: Arc<dyn PageRepository>) -> Self {
        Self { pages }
    }

    pub async fn execute(&self, actor: Actor, id: Uuid) -> Result<PageView, ApplicationError> {
        actor.require_role(Role::Viewer)?;
        let page = self
            .pages
            .find_by_id(id, actor.workspace_id)
            .await?
            .ok_or(DomainError::NotFound)?;
        let versions = self.pages.find_versions(page.id).await?;
        Ok(PageView { page, versions })
    }
}
