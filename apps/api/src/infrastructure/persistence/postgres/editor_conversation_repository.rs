use async_trait::async_trait;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

use historiador_db::postgres::editor_conversations;

use crate::domain::entity::{EditorConversation, EditorConversationMessage};
use crate::domain::error::ApplicationError;
use crate::domain::port::editor_conversation_repository::EditorConversationRepository;
use crate::domain::value::Language;

pub struct PostgresEditorConversationRepository {
    pool: PgPool,
}

impl PostgresEditorConversationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl EditorConversationRepository for PostgresEditorConversationRepository {
    async fn find(
        &self,
        page_id: Uuid,
        language: &Language,
        user_id: Uuid,
    ) -> Result<Option<EditorConversation>, ApplicationError> {
        let row =
            editor_conversations::find_by_key(&self.pool, page_id, language.as_str(), user_id)
                .await?;
        Ok(row.map(|r| EditorConversation {
            page_id: r.page_id,
            language: Language::from_trusted(r.language),
            user_id: r.user_id,
            messages: deserialize_messages(&r.messages),
            updated_at: r.updated_at,
        }))
    }

    async fn upsert(
        &self,
        page_id: Uuid,
        language: &Language,
        user_id: Uuid,
        messages: &[EditorConversationMessage],
    ) -> Result<EditorConversation, ApplicationError> {
        let payload = serialize_messages(messages);
        let row =
            editor_conversations::upsert(&self.pool, page_id, language.as_str(), user_id, &payload)
                .await?;
        Ok(EditorConversation {
            page_id: row.page_id,
            language: Language::from_trusted(row.language),
            user_id: row.user_id,
            messages: deserialize_messages(&row.messages),
            updated_at: row.updated_at,
        })
    }
}

fn serialize_messages(messages: &[EditorConversationMessage]) -> Value {
    Value::Array(
        messages
            .iter()
            .map(|m| {
                json!({
                    "role": m.role,
                    "content": m.content,
                    "ts": m.ts,
                })
            })
            .collect(),
    )
}

/// Permissively decode the JSONB array. Unknown fields are ignored and
/// malformed entries are dropped — this keeps older rows readable when
/// the shape evolves (e.g. when v1.1 adds a `mode` discriminator).
fn deserialize_messages(value: &Value) -> Vec<EditorConversationMessage> {
    let Some(array) = value.as_array() else {
        return Vec::new();
    };
    array
        .iter()
        .filter_map(|entry| {
            let role = entry.get("role")?.as_str()?.to_string();
            let content = entry.get("content")?.as_str()?.to_string();
            let ts = entry.get("ts").and_then(|v| v.as_i64()).unwrap_or(0);
            Some(EditorConversationMessage { role, content, ts })
        })
        .collect()
}
