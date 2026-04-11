//! Queries against the `sessions` table.
//!
//! Sessions store refresh tokens as sha256 hashes. A row lives from
//! `POST /auth/login` until either `POST /auth/logout` (explicit
//! deletion), `POST /auth/refresh` (rotation — old row deleted, new
//! inserted), or expiry (background cleanup via `delete_expired`).

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Session {
    pub id: Uuid,
    pub user_id: Uuid,
    pub expires_at: DateTime<Utc>,
}

/// Insert a new session row and return its id.
pub async fn insert(
    pool: &PgPool,
    user_id: Uuid,
    token_hash: &str,
    expires_at: DateTime<Utc>,
) -> anyhow::Result<Uuid> {
    let (id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO sessions (user_id, token_hash, expires_at) \
         VALUES ($1, $2, $3) \
         RETURNING id",
    )
    .bind(user_id)
    .bind(token_hash)
    .bind(expires_at)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

/// Look up a session by the sha256 hash of its refresh token.
/// Returns `None` if no matching row exists or if the token is
/// expired — callers should treat both as "not logged in".
pub async fn find_active_by_token_hash(
    pool: &PgPool,
    token_hash: &str,
) -> anyhow::Result<Option<Session>> {
    let row = sqlx::query_as::<_, Session>(
        "SELECT id, user_id, expires_at \
           FROM sessions \
          WHERE token_hash = $1 AND expires_at > now()",
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Delete a session by its token hash. Used by logout and refresh.
/// Returns the number of rows deleted (0 if the token was unknown).
pub async fn delete_by_token_hash(pool: &PgPool, token_hash: &str) -> anyhow::Result<u64> {
    let result = sqlx::query("DELETE FROM sessions WHERE token_hash = $1")
        .bind(token_hash)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

/// Background cleanup of expired rows. Not wired to a scheduler in
/// Sprint 2; safe to call opportunistically from a handler later.
pub async fn delete_expired(pool: &PgPool) -> anyhow::Result<u64> {
    let result = sqlx::query("DELETE FROM sessions WHERE expires_at <= now()")
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}
