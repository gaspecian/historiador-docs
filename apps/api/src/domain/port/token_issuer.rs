//! Token issuer / verifier port — JWT production and verification
//! without leaking `jsonwebtoken` types into the application layer.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::error::ApplicationError;
use crate::domain::value::Role;

/// Claims a use case cares about, independent of JWT wire format.
#[derive(Debug, Clone)]
pub struct AccessClaims {
    pub user_id: Uuid,
    pub workspace_id: Uuid,
    pub role: Role,
    pub expires_at: DateTime<Utc>,
}

pub trait TokenIssuer: Send + Sync {
    fn issue_access(&self, claims: &AccessClaims) -> Result<String, ApplicationError>;

    fn verify_access(&self, token: &str) -> Result<AccessClaims, ApplicationError>;
}
