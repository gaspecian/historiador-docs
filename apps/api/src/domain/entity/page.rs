use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::value::{PageStatus, Slug};

#[derive(Debug, Clone)]
pub struct Page {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub collection_id: Option<Uuid>,
    pub slug: Slug,
    pub status: PageStatus,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
