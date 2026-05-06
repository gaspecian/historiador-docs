use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use historiador_db::postgres::sessions;

use crate::domain::entity::Session;
use crate::domain::error::ApplicationError;
use crate::domain::port::session_repository::SessionRepository;

use super::mapper;

pub struct PostgresSessionRepository {
    pool: PgPool,
}

impl PostgresSessionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SessionRepository for PostgresSessionRepository {
    async fn insert(
        &self,
        user_id: Uuid,
        token_hash: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<Uuid, ApplicationError> {
        let id = sessions::insert(&self.pool, user_id, token_hash, expires_at).await?;
        Ok(id)
    }

    async fn find_active_by_token_hash(
        &self,
        token_hash: &str,
    ) -> Result<Option<Session>, ApplicationError> {
        let row = sessions::find_active_by_token_hash(&self.pool, token_hash).await?;
        Ok(row.map(mapper::session))
    }

    async fn delete_by_token_hash(&self, token_hash: &str) -> Result<u64, ApplicationError> {
        let n = sessions::delete_by_token_hash(&self.pool, token_hash).await?;
        Ok(n)
    }

    async fn delete_expired(&self) -> Result<u64, ApplicationError> {
        let n = sessions::delete_expired(&self.pool).await?;
        Ok(n)
    }
}
