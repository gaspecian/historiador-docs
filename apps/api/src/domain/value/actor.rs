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

#[cfg(test)]
mod tests {
    use super::*;

    fn actor_with(role: Role) -> Actor {
        Actor {
            user_id: Uuid::nil(),
            workspace_id: Uuid::nil(),
            role,
        }
    }

    #[test]
    fn admin_passes_every_gate() {
        let a = actor_with(Role::Admin);
        assert!(a.require_role(Role::Admin).is_ok());
        assert!(a.require_role(Role::Author).is_ok());
        assert!(a.require_role(Role::Viewer).is_ok());
    }

    #[test]
    fn author_passes_author_and_viewer() {
        let a = actor_with(Role::Author);
        assert!(a.require_role(Role::Admin).is_err());
        assert!(a.require_role(Role::Author).is_ok());
        assert!(a.require_role(Role::Viewer).is_ok());
    }

    #[test]
    fn viewer_only_passes_viewer() {
        let a = actor_with(Role::Viewer);
        assert!(a.require_role(Role::Admin).is_err());
        assert!(a.require_role(Role::Author).is_err());
        assert!(a.require_role(Role::Viewer).is_ok());
    }
}
