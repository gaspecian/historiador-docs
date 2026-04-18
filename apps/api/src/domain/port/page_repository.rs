//! Page and page-version persistence port. Application-level methods
//! only — no leaked `sqlx::Transaction`, no `anyhow::Result`.

use async_trait::async_trait;
use uuid::Uuid;

use crate::domain::entity::{Page, PageVersion};
use crate::domain::error::ApplicationError;
use crate::domain::value::{Language, PageStatus, Slug};

/// Input record for creating a new page.
#[derive(Debug, Clone)]
pub struct NewPage {
    pub workspace_id: Uuid,
    pub collection_id: Option<Uuid>,
    pub slug: Slug,
    pub created_by: Uuid,
}

/// Input record for upserting a page version.
#[derive(Debug, Clone)]
pub struct UpsertPageVersion {
    pub page_id: Uuid,
    pub language: Language,
    pub title: String,
    pub content_markdown: String,
    pub author_id: Uuid,
    pub status: PageStatus,
}

#[async_trait]
pub trait PageRepository: Send + Sync {
    async fn insert(&self, input: NewPage) -> Result<Page, ApplicationError>;

    async fn find_by_id(
        &self,
        id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Option<Page>, ApplicationError>;

    async fn list_by_collection(
        &self,
        workspace_id: Uuid,
        collection_id: Option<Uuid>,
    ) -> Result<Vec<Page>, ApplicationError>;

    async fn search_by_title(
        &self,
        workspace_id: Uuid,
        query: &str,
    ) -> Result<Vec<Page>, ApplicationError>;

    /// Set the `pages.status` column and cascade to all versions.
    async fn update_status(
        &self,
        id: Uuid,
        workspace_id: Uuid,
        status: PageStatus,
    ) -> Result<Option<Page>, ApplicationError>;

    async fn upsert_version(
        &self,
        input: UpsertPageVersion,
    ) -> Result<PageVersion, ApplicationError>;

    async fn find_versions(&self, page_id: Uuid) -> Result<Vec<PageVersion>, ApplicationError>;

    async fn find_version(
        &self,
        page_id: Uuid,
        language: &Language,
    ) -> Result<Option<PageVersion>, ApplicationError>;

    /// Every published version across the workspace — used by the
    /// reindex flow.
    async fn find_all_published_in_workspace(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<PageVersion>, ApplicationError>;
}
