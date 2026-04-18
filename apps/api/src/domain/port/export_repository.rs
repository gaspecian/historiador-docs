//! Specialized read-only port for the export flow. Returns every
//! published page version in a workspace joined with its collection
//! path and author email — a shape that doesn't belong on the
//! page repository's general surface.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::error::ApplicationError;
use crate::domain::value::Language;

#[derive(Debug, Clone)]
pub struct PublishedPageExport {
    pub page_id: Uuid,
    pub page_slug: String,
    /// Slash-joined collection name chain. `None` for pages at
    /// workspace root (not nested under any collection).
    pub collection_path: Option<String>,
    pub language: Language,
    pub title: String,
    pub content_markdown: String,
    pub author_email: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[async_trait]
pub trait ExportRepository: Send + Sync {
    async fn find_all_published(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<PublishedPageExport>, ApplicationError>;
}
