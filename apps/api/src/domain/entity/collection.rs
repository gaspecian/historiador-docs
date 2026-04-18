use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::value::Slug;

#[derive(Debug, Clone)]
pub struct Collection {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub name: String,
    pub slug: Slug,
    pub sort_order: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
