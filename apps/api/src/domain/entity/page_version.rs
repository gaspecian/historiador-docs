use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::value::{Language, PageStatus};

#[derive(Debug, Clone)]
pub struct PageVersion {
    pub id: Uuid,
    pub page_id: Uuid,
    pub language: Language,
    pub title: String,
    pub content_markdown: String,
    pub status: PageStatus,
    pub author_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
