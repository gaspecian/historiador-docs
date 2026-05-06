use std::sync::Arc;

use crate::domain::error::{ApplicationError, DomainError};
use crate::domain::port::session_repository::SessionRepository;
use crate::domain::port::token_issuer::TokenIssuer;
use crate::domain::port::user_repository::UserRepository;
use crate::infrastructure::auth::refresh_tokens as rt;

use super::login::issue_pair;
use super::tokens::IssuedTokens;

pub struct RefreshUseCase {
    users: Arc<dyn UserRepository>,
    sessions: Arc<dyn SessionRepository>,
    tokens: Arc<dyn TokenIssuer>,
}

impl RefreshUseCase {
    pub fn new(
        users: Arc<dyn UserRepository>,
        sessions: Arc<dyn SessionRepository>,
        tokens: Arc<dyn TokenIssuer>,
    ) -> Self {
        Self {
            users,
            sessions,
            tokens,
        }
    }

    pub async fn execute(&self, refresh_token: &str) -> Result<IssuedTokens, ApplicationError> {
        let hash = rt::sha256_hex(refresh_token);
        let session = self
            .sessions
            .find_active_by_token_hash(&hash)
            .await?
            .ok_or(DomainError::Forbidden)?;

        // Rotate.
        self.sessions.delete_by_token_hash(&hash).await?;

        // Reload the user for a fresh role snapshot.
        let user = self
            .users
            .find_by_id(session.user_id)
            .await?
            .ok_or(DomainError::Forbidden)?;
        if !user.active {
            return Err(DomainError::Forbidden.into());
        }

        issue_pair(
            self.tokens.as_ref(),
            self.sessions.as_ref(),
            user.id,
            user.workspace_id,
            user.role,
        )
        .await
    }
}
