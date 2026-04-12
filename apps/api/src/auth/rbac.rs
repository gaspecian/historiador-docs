//! Role hierarchy check.
//!
//! Called inline from handlers as the first thing after auth
//! extraction. Hierarchy: Admin > Author > Viewer. A user whose
//! rank is greater than or equal to the required rank is
//! authorized; otherwise the handler returns `ApiError::Forbidden`.

use historiador_db::postgres::users::Role;

use crate::auth::extractor::AuthUser;
use crate::error::ApiError;

pub fn require_role(user: &AuthUser, required: Role) -> Result<(), ApiError> {
    if user.role.rank() >= required.rank() {
        Ok(())
    } else {
        Err(ApiError::Forbidden)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn user_with(role: Role) -> AuthUser {
        AuthUser {
            user_id: Uuid::nil(),
            workspace_id: Uuid::nil(),
            role,
        }
    }

    #[test]
    fn admin_passes_every_gate() {
        let u = user_with(Role::Admin);
        assert!(require_role(&u, Role::Admin).is_ok());
        assert!(require_role(&u, Role::Author).is_ok());
        assert!(require_role(&u, Role::Viewer).is_ok());
    }

    #[test]
    fn author_passes_author_and_viewer() {
        let u = user_with(Role::Author);
        assert!(require_role(&u, Role::Admin).is_err());
        assert!(require_role(&u, Role::Author).is_ok());
        assert!(require_role(&u, Role::Viewer).is_ok());
    }

    #[test]
    fn viewer_only_passes_viewer() {
        let u = user_with(Role::Viewer);
        assert!(require_role(&u, Role::Admin).is_err());
        assert!(require_role(&u, Role::Author).is_err());
        assert!(require_role(&u, Role::Viewer).is_ok());
    }
}
