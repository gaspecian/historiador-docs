use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::domain::value::Language;

/// One message in an editor conversation. The `role` is either
/// `"user"` or `"assistant"`; `ts` is the millisecond Unix timestamp
/// the client wrote the message. No schema validation is done at the
/// domain boundary — the web client and the API evolve the shape
/// together.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorConversationMessage {
    pub role: String,
    pub content: String,
    pub ts: i64,
}

/// The persisted conversation transcript for one (page, language,
/// user) triple. Upserted on every auto-save turn.
#[derive(Debug, Clone)]
pub struct EditorConversation {
    pub page_id: Uuid,
    pub language: Language,
    pub user_id: Uuid,
    pub messages: Vec<EditorConversationMessage>,
    pub updated_at: DateTime<Utc>,
}
