//! Queries against the `workspaces` table.

use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize, utoipa::ToSchema)]
pub struct Workspace {
    pub id: Uuid,
    pub name: String,
    pub languages: Vec<String>,
    pub primary_language: String,
    pub llm_provider: String,
    pub llm_api_key_encrypted: Option<String>,
    pub mcp_bearer_token_hash: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct NewWorkspace<'a> {
    pub name: &'a str,
    pub languages: &'a [String],
    pub primary_language: &'a str,
    pub llm_provider: &'a str,
    pub llm_api_key_encrypted: &'a str,
}

/// Insert a workspace inside an open transaction, returning its id.
///
/// The `primary_language IN languages` invariant is enforced by a
/// database CHECK constraint from migration 0001; callers should
/// surface a validation error to the client before reaching this
/// point to avoid a round-trip for a known-bad payload.
pub async fn insert(
    tx: &mut Transaction<'_, Postgres>,
    w: NewWorkspace<'_>,
) -> anyhow::Result<Uuid> {
    let (id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO workspaces \
           (name, languages, primary_language, llm_provider, llm_api_key_encrypted) \
         VALUES ($1, $2, $3, $4, $5) \
         RETURNING id",
    )
    .bind(w.name)
    .bind(w.languages)
    .bind(w.primary_language)
    .bind(w.llm_provider)
    .bind(w.llm_api_key_encrypted)
    .fetch_one(&mut **tx)
    .await?;
    Ok(id)
}

/// Find a workspace by id.
pub async fn find_by_id(pool: &PgPool, id: Uuid) -> anyhow::Result<Option<Workspace>> {
    let row = sqlx::query_as::<_, Workspace>("SELECT * FROM workspaces WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

/// Update the MCP bearer token hash. Returns the number of rows
/// affected (1 if found, 0 if not).
pub async fn update_mcp_token(
    pool: &PgPool,
    workspace_id: Uuid,
    new_token_hash: &str,
) -> anyhow::Result<u64> {
    let result = sqlx::query(
        "UPDATE workspaces SET mcp_bearer_token_hash = $2 WHERE id = $1",
    )
    .bind(workspace_id)
    .bind(new_token_hash)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}
