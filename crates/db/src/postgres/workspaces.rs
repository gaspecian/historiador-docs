//! Queries against the `workspaces` table.

use sqlx::{Postgres, Transaction};
use uuid::Uuid;

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
