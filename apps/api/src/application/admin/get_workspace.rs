use std::sync::Arc;

use crate::domain::entity::Workspace;
use crate::domain::error::{ApplicationError, DomainError};
use crate::domain::port::workspace_repository::WorkspaceRepository;
use crate::domain::value::{Actor, Role};

pub struct GetWorkspaceUseCase {
    workspaces: Arc<dyn WorkspaceRepository>,
}

impl GetWorkspaceUseCase {
    pub fn new(workspaces: Arc<dyn WorkspaceRepository>) -> Self {
        Self { workspaces }
    }

    pub async fn execute(&self, actor: Actor) -> Result<Workspace, ApplicationError> {
        actor.require_role(Role::Admin)?;
        self.workspaces
            .find_by_id(actor.workspace_id)
            .await?
            .ok_or_else(|| DomainError::NotFound.into())
    }
}
