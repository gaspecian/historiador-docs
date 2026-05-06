use std::sync::Arc;

use uuid::Uuid;

use crate::domain::error::ApplicationError;
use crate::domain::port::page_repository::PageRepository;
use crate::domain::value::{Actor, Role};

use super::view::PageView;

pub struct ListPagesUseCase {
    pages: Arc<dyn PageRepository>,
}

impl ListPagesUseCase {
    pub fn new(pages: Arc<dyn PageRepository>) -> Self {
        Self { pages }
    }

    pub async fn execute(
        &self,
        actor: Actor,
        collection_id: Option<Uuid>,
    ) -> Result<Vec<PageView>, ApplicationError> {
        actor.require_role(Role::Viewer)?;
        let pages = self
            .pages
            .list_by_collection(actor.workspace_id, collection_id)
            .await?;

        let mut out = Vec::with_capacity(pages.len());
        for page in pages {
            let versions = self.pages.find_versions(page.id).await?;
            out.push(PageView { page, versions });
        }
        Ok(out)
    }
}
