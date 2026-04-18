use std::sync::Arc;

use uuid::Uuid;

use crate::domain::error::{ApplicationError, DomainError};
use crate::domain::port::export_repository::{ExportRepository, PublishedPageExport};
use crate::domain::port::workspace_repository::WorkspaceRepository;
use crate::domain::value::{Actor, Language, Role};

pub struct ExportPageUseCase {
    workspaces: Arc<dyn WorkspaceRepository>,
    export_repo: Arc<dyn ExportRepository>,
}

impl ExportPageUseCase {
    pub fn new(
        workspaces: Arc<dyn WorkspaceRepository>,
        export_repo: Arc<dyn ExportRepository>,
    ) -> Self {
        Self {
            workspaces,
            export_repo,
        }
    }

    pub async fn execute(
        &self,
        actor: Actor,
        page_id: Uuid,
        language: Option<Language>,
    ) -> Result<PublishedPageExport, ApplicationError> {
        actor.require_role(Role::Author)?;

        let ws = self
            .workspaces
            .find_by_id(actor.workspace_id)
            .await?
            .ok_or(DomainError::NotFound)?;
        let target_lang = language.unwrap_or_else(|| ws.primary_language.clone());

        let rows = self.export_repo.find_all_published(actor.workspace_id).await?;
        rows.into_iter()
            .find(|r| r.page_id == page_id && r.language.as_str() == target_lang.as_str())
            .ok_or_else(|| DomainError::NotFound.into())
    }
}
