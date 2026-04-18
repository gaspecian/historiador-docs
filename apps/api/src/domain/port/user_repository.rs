use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::entity::User;
use crate::domain::error::ApplicationError;
use crate::domain::value::{Email, Role};

#[derive(Debug, Clone)]
pub struct NewPendingUser {
    pub workspace_id: Uuid,
    pub email: Email,
    pub role: Role,
    pub invite_token_hash: String,
    pub invite_expires_at: DateTime<Utc>,
}

#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn find_by_email(
        &self,
        workspace_id: Uuid,
        email: &Email,
    ) -> Result<Option<User>, ApplicationError>;

    /// Find a user by email without scoping to a workspace. Used by
    /// login on single-workspace installs where the caller does not
    /// yet know which workspace to target.
    async fn find_by_email_any_workspace(
        &self,
        email: &Email,
    ) -> Result<Option<User>, ApplicationError>;

    async fn find_by_id(&self, id: Uuid) -> Result<Option<User>, ApplicationError>;

    async fn find_by_invite_token_hash(
        &self,
        invite_token_hash: &str,
    ) -> Result<Option<User>, ApplicationError>;

    async fn list_by_workspace(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<User>, ApplicationError>;

    async fn insert_pending(&self, input: NewPendingUser) -> Result<Uuid, ApplicationError>;

    /// Complete activation: set password hash, clear invite fields.
    async fn activate(&self, user_id: Uuid, password_hash: &str) -> Result<(), ApplicationError>;

    /// Returns true iff a row was updated.
    async fn deactivate(&self, user_id: Uuid, workspace_id: Uuid) -> Result<bool, ApplicationError>;
}
