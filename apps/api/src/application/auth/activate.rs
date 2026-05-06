use std::sync::Arc;

use chrono::Utc;

use historiador_db::password as pw;

use crate::domain::error::{ApplicationError, DomainError};
use crate::domain::port::user_repository::UserRepository;
use crate::infrastructure::auth::refresh_tokens as rt;

pub struct ActivateCommand {
    pub invite_token: String,
    pub password: String,
}

pub struct ActivateUseCase {
    users: Arc<dyn UserRepository>,
}

impl ActivateUseCase {
    pub fn new(users: Arc<dyn UserRepository>) -> Self {
        Self { users }
    }

    pub async fn execute(&self, cmd: ActivateCommand) -> Result<(), ApplicationError> {
        let hash = rt::sha256_hex(&cmd.invite_token);
        let user = self
            .users
            .find_by_invite_token_hash(&hash)
            .await?
            .ok_or(DomainError::Forbidden)?;

        let expires_at = user.invite_expires_at.ok_or(DomainError::Forbidden)?;
        if expires_at <= Utc::now() {
            return Err(DomainError::Forbidden.into());
        }

        let password_hash = pw::hash(&cmd.password).map_err(ApplicationError::Infrastructure)?;
        self.users.activate(user.id, &password_hash).await?;
        Ok(())
    }
}
