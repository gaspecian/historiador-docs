use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::entity::Session;
use crate::domain::error::ApplicationError;

#[async_trait]
pub trait SessionRepository: Send + Sync {
    async fn insert(
        &self,
        user_id: Uuid,
        token_hash: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<Uuid, ApplicationError>;

    async fn find_active_by_token_hash(
        &self,
        token_hash: &str,
    ) -> Result<Option<Session>, ApplicationError>;

    /// Returns the number of rows deleted (0 if the token was unknown).
    async fn delete_by_token_hash(&self, token_hash: &str) -> Result<u64, ApplicationError>;

    async fn delete_expired(&self) -> Result<u64, ApplicationError>;
}
