use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::entity::{VersionHistoryEntry, VersionHistorySummary};
use crate::domain::error::ApplicationError;
use crate::domain::value::Language;

#[derive(Debug, Clone)]
pub struct NewVersionSnapshot {
    pub page_id: Uuid,
    pub language: Language,
    pub title: String,
    pub content_markdown: String,
    pub is_published: bool,
    pub author_id: Option<Uuid>,
}

#[derive(Debug, Clone, Copy)]
pub struct PageRequest {
    pub page: i64,
    pub per_page: i64,
}

#[async_trait]
pub trait VersionHistoryRepository: Send + Sync {
    async fn insert(
        &self,
        input: NewVersionSnapshot,
    ) -> Result<VersionHistoryEntry, ApplicationError>;

    async fn list(
        &self,
        page_id: Uuid,
        language: &Language,
        request: PageRequest,
    ) -> Result<(Vec<VersionHistorySummary>, i64), ApplicationError>;

    async fn find_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<VersionHistoryEntry>, ApplicationError>;

    async fn has_recent_snapshot(
        &self,
        page_id: Uuid,
        language: &Language,
        seconds: i32,
    ) -> Result<bool, ApplicationError>;
}
