use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::value::{Email, Role};

#[derive(Debug, Clone)]
pub struct User {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub email: Email,
    /// `None` when the user is invited but not yet activated.
    pub password_hash: Option<String>,
    pub role: Role,
    pub active: bool,
    pub invite_token_hash: Option<String>,
    pub invite_expires_at: Option<DateTime<Utc>>,
}

impl User {
    pub fn is_activated(&self) -> bool {
        self.password_hash.is_some()
    }
}
