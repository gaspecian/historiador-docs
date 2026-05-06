use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::value::Language;

/// Full immutable snapshot of a page version at a point in time.
#[derive(Debug, Clone)]
pub struct VersionHistoryEntry {
    pub id: Uuid,
    pub page_id: Uuid,
    pub language: Language,
    pub title: String,
    pub content_markdown: String,
    pub is_published: bool,
    pub author_id: Option<Uuid>,
    pub version_number: i32,
    pub created_at: DateTime<Utc>,
}

/// Summary row (no full content) for listing.
#[derive(Debug, Clone)]
pub struct VersionHistorySummary {
    pub id: Uuid,
    pub version_number: i32,
    pub title: String,
    pub content_preview: String,
    pub is_published: bool,
    pub author_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}
