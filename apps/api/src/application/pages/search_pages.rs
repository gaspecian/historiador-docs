use std::sync::Arc;

use crate::domain::error::ApplicationError;
use crate::domain::port::page_repository::PageRepository;
use crate::domain::value::{Actor, Role};

use super::view::PageView;

pub struct SearchPagesUseCase {
    pages: Arc<dyn PageRepository>,
}

impl SearchPagesUseCase {
    pub fn new(pages: Arc<dyn PageRepository>) -> Self {
        Self { pages }
    }

    pub async fn execute(
        &self,
        actor: Actor,
        query: &str,
    ) -> Result<Vec<PageView>, ApplicationError> {
        actor.require_role(Role::Viewer)?;
        let pages = self.pages.search_by_title(actor.workspace_id, query).await?;

        let mut out = Vec::with_capacity(pages.len());
        for page in pages {
            let versions = self.pages.find_versions(page.id).await?;
            out.push(PageView { page, versions });
        }
        Ok(out)
    }
}
