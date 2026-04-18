use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::value::Language;

#[derive(Debug, Clone)]
pub struct Workspace {
    pub id: Uuid,
    pub name: String,
    pub languages: Vec<Language>,
    pub primary_language: Language,
    pub llm_provider: String,
    pub llm_api_key_encrypted: Option<String>,
    pub llm_base_url: Option<String>,
    pub generation_model: String,
    pub embedding_model: String,
    pub mcp_bearer_token_hash: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
