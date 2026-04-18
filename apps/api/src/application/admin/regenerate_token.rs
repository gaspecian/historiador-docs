use std::sync::Arc;

use crate::infrastructure::auth::refresh_tokens as rt;
use crate::domain::error::{ApplicationError, DomainError};
use crate::domain::port::workspace_repository::WorkspaceRepository;
use crate::domain::value::{Actor, Role};

pub struct RegenerateTokenUseCase {
    workspaces: Arc<dyn WorkspaceRepository>,
}

impl RegenerateTokenUseCase {
    pub fn new(workspaces: Arc<dyn WorkspaceRepository>) -> Self {
        Self { workspaces }
    }

    /// Returns the plaintext bearer token. Caller must surface it to
    /// the admin exactly once — the server stores only the hash.
    pub async fn execute(&self, actor: Actor) -> Result<String, ApplicationError> {
        actor.require_role(Role::Admin)?;
        let (plaintext, hash) = rt::generate();
        let updated = self
            .workspaces
            .update_mcp_token(actor.workspace_id, &hash)
            .await?;
        if !updated {
            return Err(DomainError::NotFound.into());
        }
        Ok(plaintext)
    }
}
