use std::sync::Arc;

use chrono::{Duration, Utc};

use historiador_db::password as pw;

use crate::domain::error::{ApplicationError, DomainError};
use crate::domain::port::session_repository::SessionRepository;
use crate::domain::port::token_issuer::{AccessClaims, TokenIssuer};
use crate::domain::port::user_repository::UserRepository;
use crate::domain::value::Email;
use crate::infrastructure::auth::jwt::ACCESS_TOKEN_TTL_SECONDS;
use crate::infrastructure::auth::refresh_tokens as rt;

use super::tokens::IssuedTokens;

pub struct LoginCommand {
    pub email: Email,
    pub password: String,
}

pub struct LoginUseCase {
    users: Arc<dyn UserRepository>,
    sessions: Arc<dyn SessionRepository>,
    tokens: Arc<dyn TokenIssuer>,
}

impl LoginUseCase {
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

    pub async fn execute(&self, cmd: LoginCommand) -> Result<IssuedTokens, ApplicationError> {
        let user = self
            .users
            .find_by_email_any_workspace(&cmd.email)
            .await?
            .ok_or_else(|| ApplicationError::Domain(DomainError::Forbidden))?;

        if !user.active {
            return Err(DomainError::Forbidden.into());
        }
        let stored_hash = user
            .password_hash
            .as_deref()
            .ok_or(DomainError::Forbidden)?;
        let matches = pw::verify(&cmd.password, stored_hash)
            .map_err(|_| ApplicationError::Domain(DomainError::Forbidden))?;
        if !matches {
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

pub(crate) async fn issue_pair(
    tokens: &dyn TokenIssuer,
    sessions: &dyn SessionRepository,
    user_id: uuid::Uuid,
    workspace_id: uuid::Uuid,
    role: crate::domain::value::Role,
) -> Result<IssuedTokens, ApplicationError> {
    let expires_at = Utc::now() + Duration::seconds(ACCESS_TOKEN_TTL_SECONDS);
    let access_token = tokens.issue_access(&AccessClaims {
        user_id,
        workspace_id,
        role,
        expires_at,
    })?;

    let (refresh_plaintext, refresh_hash) = rt::generate();
    let refresh_expires_at = Utc::now() + Duration::days(rt::REFRESH_TOKEN_TTL_DAYS);
    sessions
        .insert(user_id, &refresh_hash, refresh_expires_at)
        .await?;

    Ok(IssuedTokens {
        access_token,
        refresh_token: refresh_plaintext,
        expires_in_seconds: ACCESS_TOKEN_TTL_SECONDS,
    })
}
