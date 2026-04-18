//! Queries against the `users` table.
//!
//! After migration 0002, rows are in one of two states:
//! - **Activated**: `password_hash IS NOT NULL`, `invite_token_hash IS NULL`
//! - **Pending**:  `password_hash IS NULL`, `invite_token_hash IS NOT NULL`
//!
//! The `users_invite_xor_password` CHECK constraint enforces this
//! split at the database layer, so application code cannot produce a
//! row that is both pending and activated.

use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

#[derive(
    Debug,
    Clone,
    Copy,
    sqlx::Type,
    serde::Serialize,
    serde::Deserialize,
    PartialEq,
    Eq,
    utoipa::ToSchema,
)]
#[sqlx(type_name = "user_role", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Admin,
    Author,
    Viewer,
}

impl Role {
    /// Role ordering for hierarchy checks: Admin > Author > Viewer.
    /// A user whose role `>= required` is authorized.
    pub fn rank(&self) -> u8 {
        match self {
            Role::Viewer => 0,
            Role::Author => 1,
            Role::Admin => 2,
        }
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub email: String,
    pub password_hash: Option<String>,
    pub role: Role,
    pub active: bool,
    pub invite_token_hash: Option<String>,
    pub invite_expires_at: Option<DateTime<Utc>>,
}

/// Insert an already-activated admin user inside an open transaction.
/// Used only by the setup wizard.
pub async fn insert_admin(
    tx: &mut Transaction<'_, Postgres>,
    workspace_id: Uuid,
    email: &str,
    password_hash: &str,
) -> anyhow::Result<Uuid> {
    let (id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO users (workspace_id, email, password_hash, role, active) \
         VALUES ($1, $2, $3, 'admin', TRUE) \
         RETURNING id",
    )
    .bind(workspace_id)
    .bind(email)
    .bind(password_hash)
    .fetch_one(&mut **tx)
    .await?;
    Ok(id)
}

/// Insert a pending (invited-but-not-yet-activated) user.
pub async fn insert_pending(
    pool: &PgPool,
    workspace_id: Uuid,
    email: &str,
    role: Role,
    invite_token_hash: &str,
    invite_expires_at: DateTime<Utc>,
) -> anyhow::Result<Uuid> {
    let (id,): (Uuid,) = sqlx::query_as(
        "INSERT INTO users \
           (workspace_id, email, role, active, invite_token_hash, invite_expires_at) \
         VALUES ($1, $2, $3, FALSE, $4, $5) \
         RETURNING id",
    )
    .bind(workspace_id)
    .bind(email)
    .bind(role)
    .bind(invite_token_hash)
    .bind(invite_expires_at)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

/// Look up a user by email within a workspace. Returns `None` if no
/// matching row exists; does not distinguish "no user" from "pending"
/// — callers that care check `password_hash.is_some()`.
pub async fn find_by_email(
    pool: &PgPool,
    workspace_id: Uuid,
    email: &str,
) -> anyhow::Result<Option<User>> {
    let row = sqlx::query_as::<_, User>(
        "SELECT id, workspace_id, email, password_hash, role, active, \
                invite_token_hash, invite_expires_at \
           FROM users \
          WHERE workspace_id = $1 AND email = $2",
    )
    .bind(workspace_id)
    .bind(email)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Look up a user by email across all workspaces. Used by login on
/// single-workspace installs where the caller does not yet know
/// which workspace the email belongs to. Callers that do know the
/// workspace should use [`find_by_email`] instead.
pub async fn find_by_email_any_workspace(
    pool: &PgPool,
    email: &str,
) -> anyhow::Result<Option<User>> {
    let row = sqlx::query_as::<_, User>(
        "SELECT id, workspace_id, email, password_hash, role, active, \
                invite_token_hash, invite_expires_at \
           FROM users \
          WHERE email = $1",
    )
    .bind(email)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Look up a user by id.
pub async fn find_by_id(pool: &PgPool, id: Uuid) -> anyhow::Result<Option<User>> {
    let row = sqlx::query_as::<_, User>(
        "SELECT id, workspace_id, email, password_hash, role, active, \
                invite_token_hash, invite_expires_at \
           FROM users WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Look up a pending user by the sha256 hash of their invite token.
pub async fn find_by_invite_token_hash(
    pool: &PgPool,
    invite_token_hash: &str,
) -> anyhow::Result<Option<User>> {
    let row = sqlx::query_as::<_, User>(
        "SELECT id, workspace_id, email, password_hash, role, active, \
                invite_token_hash, invite_expires_at \
           FROM users \
          WHERE invite_token_hash = $1",
    )
    .bind(invite_token_hash)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// List all users in a workspace, ordered by creation date.
pub async fn list_by_workspace(pool: &PgPool, workspace_id: Uuid) -> anyhow::Result<Vec<User>> {
    let rows = sqlx::query_as::<_, User>(
        "SELECT id, workspace_id, email, password_hash, role, active, \
                invite_token_hash, invite_expires_at \
           FROM users \
          WHERE workspace_id = $1 \
          ORDER BY created_at",
    )
    .bind(workspace_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Deactivate a user by setting `active = FALSE`. Returns the number
/// of rows affected (1 if found, 0 if not).
pub async fn deactivate(pool: &PgPool, user_id: Uuid, workspace_id: Uuid) -> anyhow::Result<u64> {
    let result = sqlx::query(
        "UPDATE users SET active = FALSE \
         WHERE id = $1 AND workspace_id = $2",
    )
    .bind(user_id)
    .bind(workspace_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

/// Complete the invite flow: set a password, clear the invite token,
/// activate the user. Runs inside a transaction so the CHECK
/// constraint (`password XOR invite_token`) is never violated.
pub async fn activate(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Uuid,
    password_hash: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE users \
            SET password_hash = $2, \
                invite_token_hash = NULL, \
                invite_expires_at = NULL, \
                active = TRUE \
          WHERE id = $1",
    )
    .bind(user_id)
    .bind(password_hash)
    .execute(&mut **tx)
    .await?;
    Ok(())
}
