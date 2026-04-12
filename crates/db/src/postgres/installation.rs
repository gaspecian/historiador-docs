//! Queries against the `installation` singleton table.
//!
//! The row with `id = 1` always exists (seeded by migration 0002).
//! `setup_complete` flips from FALSE to TRUE exactly once, inside
//! the setup wizard transaction.

use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Installation {
    pub setup_complete: bool,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Read the singleton row. The row is guaranteed to exist.
pub async fn get(pool: &PgPool) -> anyhow::Result<Installation> {
    let row = sqlx::query_as::<_, Installation>(
        "SELECT setup_complete, completed_at FROM installation WHERE id = 1",
    )
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// Mark setup complete inside an open transaction. Idempotent: calling
/// this twice is not an error, but the caller should gate on the
/// current flag to avoid overwriting `completed_at`.
pub async fn mark_complete(tx: &mut Transaction<'_, Postgres>) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE installation \
         SET setup_complete = TRUE, completed_at = now() \
         WHERE id = 1",
    )
    .execute(&mut **tx)
    .await?;
    Ok(())
}
