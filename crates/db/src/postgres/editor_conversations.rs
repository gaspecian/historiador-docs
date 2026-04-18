//! Queries against the `editor_conversations` table (Sprint 10).
//!
//! One row per (page_id, language, user_id). The `messages` column is
//! a JSONB array of `{role, content, ts}` objects; no schema validation
//! is enforced in the DB layer — it's the application's responsibility
//! to keep the shape stable.

use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

/// Full row from the `editor_conversations` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct EditorConversationRow {
    pub page_id: Uuid,
    pub language: String,
    pub user_id: Uuid,
    pub messages: Value,
    pub updated_at: DateTime<Utc>,
}

/// Upsert the conversation transcript for a (page, language, user)
/// triple. `updated_at` is refreshed on every call.
pub async fn upsert(
    pool: &PgPool,
    page_id: Uuid,
    language: &str,
    user_id: Uuid,
    messages: &Value,
) -> anyhow::Result<EditorConversationRow> {
    let row = sqlx::query_as::<_, EditorConversationRow>(
        "INSERT INTO editor_conversations (page_id, language, user_id, messages, updated_at)
         VALUES ($1, $2, $3, $4, now())
         ON CONFLICT (page_id, language, user_id) DO UPDATE
         SET messages = EXCLUDED.messages,
             updated_at = now()
         RETURNING *",
    )
    .bind(page_id)
    .bind(language)
    .bind(user_id)
    .bind(messages)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// Fetch the conversation transcript for a (page, language, user)
/// triple. Returns `None` when the author has never saved a turn.
pub async fn find_by_key(
    pool: &PgPool,
    page_id: Uuid,
    language: &str,
    user_id: Uuid,
) -> anyhow::Result<Option<EditorConversationRow>> {
    let row = sqlx::query_as::<_, EditorConversationRow>(
        "SELECT page_id, language, user_id, messages, updated_at
         FROM editor_conversations
         WHERE page_id = $1 AND language = $2 AND user_id = $3",
    )
    .bind(page_id)
    .bind(language)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Delete the conversation transcript (used when the author abandons a
/// draft or the UI wants to reset the thread). CASCADE on `pages` /
/// `users` covers the common-case cleanup.
pub async fn delete_by_key(
    pool: &PgPool,
    page_id: Uuid,
    language: &str,
    user_id: Uuid,
) -> anyhow::Result<u64> {
    let res = sqlx::query(
        "DELETE FROM editor_conversations
         WHERE page_id = $1 AND language = $2 AND user_id = $3",
    )
    .bind(page_id)
    .bind(language)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(res.rows_affected())
}
