use std::sync::Arc;

use crate::domain::entity::Workspace;
use crate::domain::error::{ApplicationError, DomainError};
use crate::domain::port::export_repository::{ExportRepository, PublishedPageExport};
use crate::domain::port::workspace_repository::WorkspaceRepository;
use crate::domain::value::{Actor, Role};

pub struct WorkspaceExportView {
    pub workspace: Workspace,
    pub pages: Vec<PublishedPageExport>,
}

pub struct ExportWorkspaceUseCase {
    workspaces: Arc<dyn WorkspaceRepository>,
    export_repo: Arc<dyn ExportRepository>,
}

impl ExportWorkspaceUseCase {
    pub fn new(
        workspaces: Arc<dyn WorkspaceRepository>,
        export_repo: Arc<dyn ExportRepository>,
    ) -> Self {
        Self {
            workspaces,
            export_repo,
        }
    }

    pub async fn execute(&self, actor: Actor) -> Result<WorkspaceExportView, ApplicationError> {
        actor.require_role(Role::Admin)?;
        let workspace = self
            .workspaces
            .find_by_id(actor.workspace_id)
            .await?
            .ok_or(DomainError::NotFound)?;
        let pages = self
            .export_repo
            .find_all_published(actor.workspace_id)
            .await?;
        Ok(WorkspaceExportView { workspace, pages })
    }
}
