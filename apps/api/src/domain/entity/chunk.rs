use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::value::Language;

#[derive(Debug, Clone)]
pub struct Chunk {
    pub id: Uuid,
    pub page_version_id: Uuid,
    pub heading_path: Vec<String>,
    pub section_index: i32,
    pub token_count: i32,
    pub oversized: bool,
    pub language: Language,
    pub vexfs_ref: String,
    pub created_at: DateTime<Utc>,
}

/// Input record for persisting a new chunk.
#[derive(Debug, Clone)]
pub struct NewChunk {
    pub page_version_id: Uuid,
    pub heading_path: Vec<String>,
    pub section_index: i32,
    pub token_count: i32,
    pub oversized: bool,
    pub language: Language,
    pub vexfs_ref: String,
}
