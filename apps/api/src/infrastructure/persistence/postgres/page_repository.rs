use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use historiador_db::postgres::{page_versions, pages};

use crate::domain::entity::{Page, PageVersion};
use crate::domain::error::ApplicationError;
use crate::domain::port::page_repository::{NewPage, PageRepository, UpsertPageVersion};
use crate::domain::value::{Language, PageStatus};

use super::mapper;

pub struct PostgresPageRepository {
    pool: PgPool,
}

impl PostgresPageRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PageRepository for PostgresPageRepository {
    async fn insert(&self, input: NewPage) -> Result<Page, ApplicationError> {
        let row = pages::insert(
            &self.pool,
            input.workspace_id,
            input.collection_id,
            input.slug.as_str(),
            input.created_by,
        )
        .await?;
        Ok(mapper::page(row))
    }

    async fn find_by_id(
        &self,
        id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Option<Page>, ApplicationError> {
        let row = pages::find_by_id(&self.pool, id, workspace_id).await?;
        Ok(row.map(mapper::page))
    }

    async fn list_by_collection(
        &self,
        workspace_id: Uuid,
        collection_id: Option<Uuid>,
    ) -> Result<Vec<Page>, ApplicationError> {
        let rows = pages::list_by_collection(&self.pool, workspace_id, collection_id).await?;
        Ok(rows.into_iter().map(mapper::page).collect())
    }

    async fn search_by_title(
        &self,
        workspace_id: Uuid,
        query: &str,
    ) -> Result<Vec<Page>, ApplicationError> {
        let rows = pages::search(&self.pool, workspace_id, query).await?;
        Ok(rows.into_iter().map(mapper::page).collect())
    }

    async fn update_status(
        &self,
        id: Uuid,
        workspace_id: Uuid,
        status: PageStatus,
    ) -> Result<Option<Page>, ApplicationError> {
        let db_status = mapper::page_status_to_db(status);
        let row = pages::update_status(&self.pool, id, workspace_id, db_status).await?;
        if row.is_some() {
            // Cascade to every version of the page.
            page_versions::update_status_all(&self.pool, id, db_status).await?;
        }
        Ok(row.map(mapper::page))
    }

    async fn upsert_version(
        &self,
        input: UpsertPageVersion,
    ) -> Result<PageVersion, ApplicationError> {
        let row = page_versions::upsert(
            &self.pool,
            input.page_id,
            input.language.as_str(),
            &input.title,
            &input.content_markdown,
            input.author_id,
            mapper::page_status_to_db(input.status),
        )
        .await?;
        Ok(mapper::page_version(row))
    }

    async fn find_versions(&self, page_id: Uuid) -> Result<Vec<PageVersion>, ApplicationError> {
        let rows = page_versions::find_by_page(&self.pool, page_id).await?;
        Ok(rows.into_iter().map(mapper::page_version).collect())
    }

    async fn find_version(
        &self,
        page_id: Uuid,
        language: &Language,
    ) -> Result<Option<PageVersion>, ApplicationError> {
        let row =
            page_versions::find_by_page_and_language(&self.pool, page_id, language.as_str())
                .await?;
        Ok(row.map(mapper::page_version))
    }

    async fn find_all_published_in_workspace(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<PageVersion>, ApplicationError> {
        let rows = page_versions::find_all_published_in_workspace(&self.pool, workspace_id).await?;
        Ok(rows.into_iter().map(mapper::page_version).collect())
    }
}
