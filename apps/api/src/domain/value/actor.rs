//! The authenticated caller of a use case. Carries identity + role so
//! use cases can perform authorization without touching the HTTP layer.

use uuid::Uuid;

use crate::domain::error::{ApplicationError, DomainError};
use crate::domain::value::Role;

#[derive(Debug, Clone, Copy)]
pub struct Actor {
    pub user_id: Uuid,
    pub workspace_id: Uuid,
    pub role: Role,
}

impl Actor {
    pub fn require_role(&self, required: Role) -> Result<(), ApplicationError> {
        if self.role.at_least(required) {
            Ok(())
        } else {
            Err(DomainError::Forbidden.into())
        }
    }
}
