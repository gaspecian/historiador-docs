use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};

use crate::domain::error::{ApplicationError, DomainError};
use crate::domain::port::user_repository::{NewPendingUser, UserRepository};
use crate::domain::value::{Actor, Email, Role};
use crate::infrastructure::auth::refresh_tokens::{self as rt, INVITE_TOKEN_TTL_DAYS};

pub struct InviteUserCommand {
    pub email: Email,
    pub role: Role,
}

pub struct InviteUserResult {
    pub user_id: uuid::Uuid,
    /// Plaintext invite token. Shown once; caller is responsible for
    /// building an activation URL around it.
    pub invite_token: String,
    pub expires_at: DateTime<Utc>,
}

pub struct InviteUserUseCase {
    users: Arc<dyn UserRepository>,
}

impl InviteUserUseCase {
    pub fn new(users: Arc<dyn UserRepository>) -> Self {
        Self { users }
    }

    pub async fn execute(
        &self,
        actor: Actor,
        cmd: InviteUserCommand,
    ) -> Result<InviteUserResult, ApplicationError> {
        actor.require_role(Role::Admin)?;

        if self
            .users
            .find_by_email(actor.workspace_id, &cmd.email)
            .await?
            .is_some()
        {
            return Err(DomainError::Conflict(
                "a user with this email already exists in the workspace".into(),
            )
            .into());
        }

        let (plaintext, hash) = rt::generate();
        let expires_at = Utc::now() + Duration::days(INVITE_TOKEN_TTL_DAYS);

        let user_id = self
            .users
            .insert_pending(NewPendingUser {
                workspace_id: actor.workspace_id,
                email: cmd.email,
                role: cmd.role,
                invite_token_hash: hash,
                invite_expires_at: expires_at,
            })
            .await?;

        Ok(InviteUserResult {
            user_id,
            invite_token: plaintext,
            expires_at,
        })
    }
}
