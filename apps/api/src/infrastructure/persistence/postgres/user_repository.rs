use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use historiador_db::postgres::users;

use crate::domain::entity::User;
use crate::domain::error::ApplicationError;
use crate::domain::port::user_repository::{NewPendingUser, UserRepository};
use crate::domain::value::Email;

use super::mapper;

pub struct PostgresUserRepository {
    pool: PgPool,
}

impl PostgresUserRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserRepository for PostgresUserRepository {
    async fn find_by_email(
        &self,
        workspace_id: Uuid,
        email: &Email,
    ) -> Result<Option<User>, ApplicationError> {
        let row = users::find_by_email(&self.pool, workspace_id, email.as_str()).await?;
        Ok(row.map(mapper::user))
    }

    async fn find_by_email_any_workspace(
        &self,
        email: &Email,
    ) -> Result<Option<User>, ApplicationError> {
        let row = users::find_by_email_any_workspace(&self.pool, email.as_str()).await?;
        Ok(row.map(mapper::user))
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<User>, ApplicationError> {
        let row = users::find_by_id(&self.pool, id).await?;
        Ok(row.map(mapper::user))
    }

    async fn find_by_invite_token_hash(
        &self,
        invite_token_hash: &str,
    ) -> Result<Option<User>, ApplicationError> {
        let row = users::find_by_invite_token_hash(&self.pool, invite_token_hash).await?;
        Ok(row.map(mapper::user))
    }

    async fn list_by_workspace(&self, workspace_id: Uuid) -> Result<Vec<User>, ApplicationError> {
        let rows = users::list_by_workspace(&self.pool, workspace_id).await?;
        Ok(rows.into_iter().map(mapper::user).collect())
    }

    async fn insert_pending(&self, input: NewPendingUser) -> Result<Uuid, ApplicationError> {
        let id = users::insert_pending(
            &self.pool,
            input.workspace_id,
            input.email.as_str(),
            mapper::role_to_db(input.role),
            &input.invite_token_hash,
            input.invite_expires_at,
        )
        .await?;
        Ok(id)
    }

    async fn activate(&self, user_id: Uuid, password_hash: &str) -> Result<(), ApplicationError> {
        let mut tx = self.pool.begin().await.map_err(anyhow::Error::from)?;
        users::activate(&mut tx, user_id, password_hash).await?;
        tx.commit().await.map_err(anyhow::Error::from)?;
        Ok(())
    }

    async fn deactivate(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<bool, ApplicationError> {
        let affected = users::deactivate(&self.pool, user_id, workspace_id).await?;
        Ok(affected > 0)
    }
}
