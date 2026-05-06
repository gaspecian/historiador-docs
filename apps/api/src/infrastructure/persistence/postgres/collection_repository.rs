use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use historiador_db::postgres::collections;

use crate::domain::entity::Collection;
use crate::domain::error::ApplicationError;
use crate::domain::port::collection_repository::{
    CollectionPatch, CollectionRepository, NewCollection,
};

use super::mapper;

pub struct PostgresCollectionRepository {
    pool: PgPool,
}

impl PostgresCollectionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl CollectionRepository for PostgresCollectionRepository {
    async fn insert(&self, input: NewCollection) -> Result<Collection, ApplicationError> {
        let row = collections::insert(
            &self.pool,
            input.workspace_id,
            input.parent_id,
            &input.name,
            input.slug.as_str(),
        )
        .await?;
        Ok(mapper::collection(row))
    }

    async fn find_by_id(
        &self,
        id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Option<Collection>, ApplicationError> {
        let row = collections::find_by_id(&self.pool, id, workspace_id).await?;
        Ok(row.map(mapper::collection))
    }

    async fn list_by_workspace(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<Collection>, ApplicationError> {
        let rows = collections::list_by_workspace(&self.pool, workspace_id).await?;
        Ok(rows.into_iter().map(mapper::collection).collect())
    }

    async fn update(
        &self,
        id: Uuid,
        workspace_id: Uuid,
        patch: CollectionPatch,
    ) -> Result<Option<Collection>, ApplicationError> {
        // The underlying helper expects `Option<&str>` values and
        // `Option<Option<Uuid>>` for nullable parent updates.
        let slug_string = patch.slug.map(|s| s.into_string());
        let row = collections::update(
            &self.pool,
            id,
            workspace_id,
            patch.name.as_deref(),
            slug_string.as_deref(),
            patch.parent_id,
        )
        .await?;
        Ok(row.map(mapper::collection))
    }

    async fn delete(&self, id: Uuid, workspace_id: Uuid) -> Result<bool, ApplicationError> {
        let affected = collections::delete_cascade(&self.pool, id, workspace_id).await?;
        Ok(affected > 0)
    }
}
