use std::sync::Arc;

use uuid::Uuid;

use crate::domain::error::{ApplicationError, DomainError};
use crate::domain::port::user_repository::UserRepository;
use crate::domain::value::{Actor, Role};

pub struct DeactivateUserUseCase {
    users: Arc<dyn UserRepository>,
}

impl DeactivateUserUseCase {
    pub fn new(users: Arc<dyn UserRepository>) -> Self {
        Self { users }
    }

    pub async fn execute(&self, actor: Actor, user_id: Uuid) -> Result<(), ApplicationError> {
        actor.require_role(Role::Admin)?;
        let affected = self.users.deactivate(user_id, actor.workspace_id).await?;
        if !affected {
            return Err(DomainError::NotFound.into());
        }
        Ok(())
    }
}
