use std::sync::Arc;

use uuid::Uuid;

use crate::domain::error::{ApplicationError, DomainError};
use crate::domain::port::page_repository::PageRepository;
use crate::domain::port::workspace_repository::WorkspaceRepository;
use crate::domain::value::{Actor, Role};

use super::view::PageVersionsView;

pub struct GetPageVersionsUseCase {
    pages: Arc<dyn PageRepository>,
    workspaces: Arc<dyn WorkspaceRepository>,
}

impl GetPageVersionsUseCase {
    pub fn new(
        pages: Arc<dyn PageRepository>,
        workspaces: Arc<dyn WorkspaceRepository>,
    ) -> Self {
        Self { pages, workspaces }
    }

    pub async fn execute(
        &self,
        actor: Actor,
        page_id: Uuid,
    ) -> Result<PageVersionsView, ApplicationError> {
        actor.require_role(Role::Viewer)?;

        let page = self
            .pages
            .find_by_id(page_id, actor.workspace_id)
            .await?
            .ok_or(DomainError::NotFound)?;

        let ws = self
            .workspaces
            .find_by_id(actor.workspace_id)
            .await?
            .ok_or(DomainError::NotFound)?;

        let versions = self.pages.find_versions(page.id).await?;

        let existing: std::collections::HashSet<String> = versions
            .iter()
            .map(|v| v.language.as_str().to_string())
            .collect();
        let missing_languages = ws
            .languages
            .iter()
            .filter(|lang| !existing.contains(lang.as_str()))
            .cloned()
            .collect();

        Ok(PageVersionsView {
            page,
            workspace_languages: ws.languages,
            primary_language: ws.primary_language,
            versions,
            missing_languages,
        })
    }
}
