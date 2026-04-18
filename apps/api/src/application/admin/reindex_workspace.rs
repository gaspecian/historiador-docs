//! Reindex returns the set of page versions to re-embed. The handler
//! spawns the actual chunking (tokio::spawn) using a freshly-built
//! embedding client that reflects the current workspace config.

use std::sync::Arc;

use crate::domain::entity::{PageVersion, Workspace};
use crate::domain::error::{ApplicationError, DomainError};
use crate::domain::port::page_repository::PageRepository;
use crate::domain::port::workspace_repository::WorkspaceRepository;
use crate::domain::value::{Actor, Role};

pub struct ReindexWorkspaceUseCase {
    workspaces: Arc<dyn WorkspaceRepository>,
    pages: Arc<dyn PageRepository>,
}

pub struct ReindexPlan {
    pub workspace: Workspace,
    pub versions: Vec<PageVersion>,
}

impl ReindexPlan {
    pub fn scheduled(&self) -> i64 {
        self.versions.len() as i64
    }
}

impl ReindexWorkspaceUseCase {
    pub fn new(
        workspaces: Arc<dyn WorkspaceRepository>,
        pages: Arc<dyn PageRepository>,
    ) -> Self {
        Self { workspaces, pages }
    }

    pub async fn execute(&self, actor: Actor) -> Result<ReindexPlan, ApplicationError> {
        actor.require_role(Role::Admin)?;
        let workspace = self
            .workspaces
            .find_by_id(actor.workspace_id)
            .await?
            .ok_or(DomainError::NotFound)?;
        let versions = self
            .pages
            .find_all_published_in_workspace(actor.workspace_id)
            .await?;
        Ok(ReindexPlan {
            workspace,
            versions,
        })
    }
}
