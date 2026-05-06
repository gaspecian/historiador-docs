use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use historiador_db::postgres::page_version_history;

use crate::domain::entity::{VersionHistoryEntry, VersionHistorySummary};
use crate::domain::error::ApplicationError;
use crate::domain::port::version_history_repository::{
    NewVersionSnapshot, PageRequest, VersionHistoryRepository,
};
use crate::domain::value::Language;

use super::mapper;

pub struct PostgresVersionHistoryRepository {
    pool: PgPool,
}

impl PostgresVersionHistoryRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl VersionHistoryRepository for PostgresVersionHistoryRepository {
    async fn insert(
        &self,
        input: NewVersionSnapshot,
    ) -> Result<VersionHistoryEntry, ApplicationError> {
        let row = page_version_history::insert(
            &self.pool,
            input.page_id,
            input.language.as_str(),
            &input.title,
            &input.content_markdown,
            input.is_published,
            input.author_id,
        )
        .await?;
        Ok(mapper::version_history_entry(row))
    }

    async fn list(
        &self,
        page_id: Uuid,
        language: &Language,
        request: PageRequest,
    ) -> Result<(Vec<VersionHistorySummary>, i64), ApplicationError> {
        let (rows, total) = page_version_history::list_by_page_and_language(
            &self.pool,
            page_id,
            language.as_str(),
            request.page,
            request.per_page,
        )
        .await?;
        let summaries = rows
            .into_iter()
            .map(mapper::version_history_summary)
            .collect();
        Ok((summaries, total))
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<VersionHistoryEntry>, ApplicationError> {
        let row = page_version_history::find_by_id(&self.pool, id).await?;
        Ok(row.map(mapper::version_history_entry))
    }

    async fn has_recent_snapshot(
        &self,
        page_id: Uuid,
        language: &Language,
        seconds: i32,
    ) -> Result<bool, ApplicationError> {
        let exists = page_version_history::has_recent_snapshot(
            &self.pool,
            page_id,
            language.as_str(),
            seconds,
        )
        .await?;
        Ok(exists)
    }
}
