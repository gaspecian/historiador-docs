//! JWT-backed `TokenIssuer` adapter. Thin wrapper over the existing
//! [`crate::auth::jwt`] helpers; the application layer never sees
//! `jsonwebtoken` types.

use chrono::{DateTime, TimeZone, Utc};

use historiador_db::postgres::users::Role as DbRole;

use crate::auth::jwt::{decode_token, encode_token, Claims};
use crate::domain::error::{ApplicationError, DomainError};
use crate::domain::port::token_issuer::{AccessClaims, TokenIssuer};
use crate::domain::value::Role;

pub struct JwtTokenIssuer {
    secret: Vec<u8>,
}

impl JwtTokenIssuer {
    pub fn new(secret: Vec<u8>) -> Self {
        Self { secret }
    }
}

impl TokenIssuer for JwtTokenIssuer {
    fn issue_access(&self, claims: &AccessClaims) -> Result<String, ApplicationError> {
        // Reuse `Claims::new` so jti/iat/exp policy stays centralized,
        // then override exp to the caller-provided deadline.
        let mut jwt_claims = Claims::new(
            claims.user_id,
            claims.workspace_id,
            role_to_db(claims.role),
        );
        jwt_claims.exp = claims.expires_at.timestamp();

        encode_token(&jwt_claims, &self.secret).map_err(ApplicationError::Infrastructure)
    }

    fn verify_access(&self, token: &str) -> Result<AccessClaims, ApplicationError> {
        let claims = decode_token(token, &self.secret)
            .map_err(|_| ApplicationError::Domain(DomainError::Forbidden))?;
        let expires_at = Utc
            .timestamp_opt(claims.exp, 0)
            .single()
            .ok_or_else(|| DomainError::Validation("invalid token expiry".into()))?;
        Ok(AccessClaims {
            user_id: claims.sub,
            workspace_id: claims.wsid,
            role: role_from_db(claims.role),
            expires_at,
        })
    }
}

fn role_to_db(role: Role) -> DbRole {
    match role {
        Role::Admin => DbRole::Admin,
        Role::Author => DbRole::Author,
        Role::Viewer => DbRole::Viewer,
    }
}

fn role_from_db(role: DbRole) -> Role {
    match role {
        DbRole::Admin => Role::Admin,
        DbRole::Author => Role::Author,
        DbRole::Viewer => Role::Viewer,
    }
}

/// Silences the unused-import warning on `DateTime` when this file is
/// compiled in isolation — `DateTime<Utc>` appears as a type in the
/// port trait signature and is needed for docs/tests.
#[allow(dead_code)]
type _Keep = DateTime<Utc>;
