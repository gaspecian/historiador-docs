use std::sync::Arc;

use crate::domain::entity::User;
use crate::domain::error::ApplicationError;
use crate::domain::port::user_repository::UserRepository;
use crate::domain::value::{Actor, Role};

pub struct ListUsersUseCase {
    users: Arc<dyn UserRepository>,
}

impl ListUsersUseCase {
    pub fn new(users: Arc<dyn UserRepository>) -> Self {
        Self { users }
    }

    pub async fn execute(&self, actor: Actor) -> Result<Vec<User>, ApplicationError> {
        actor.require_role(Role::Admin)?;
        self.users.list_by_workspace(actor.workspace_id).await
    }
}
