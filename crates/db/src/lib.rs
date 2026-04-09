//! `historiador_db` — shared database clients for Historiador Doc.
//!
//! Consumed by both `historiador_api` and `historiador_mcp`.
//!
//! # Invariant (ADR-003)
//!
//! Migrations are applied **only** by `historiador_api` on boot. The MCP
//! server must never call [`run_migrations`] — the readonly Postgres role
//! lacks DDL privileges and MCP has no business owning schema evolution.
//! The call would fail at runtime, but the right time to enforce this is
//! at code review, not at a crashing container.

pub mod postgres;
pub mod vector_store;

use sqlx::PgPool;
use std::time::Duration;

/// Statically embedded SQL migrations, compiled from `./migrations` at
/// build time. Applied exclusively by the API server on boot.
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Open a Postgres connection pool against the given URL.
///
/// The pool is configured for the typical workspace load (low
/// tens of concurrent requests). Tune `max_connections` upward for
/// workspaces with heavy MCP query traffic.
pub async fn connect(database_url: &str) -> anyhow::Result<PgPool> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(16)
        .acquire_timeout(Duration::from_secs(5))
        .connect(database_url)
        .await?;
    Ok(pool)
}

/// Apply all embedded migrations. **Only `historiador_api` should call
/// this.** See the crate-level invariant.
pub async fn run_migrations(pool: &PgPool) -> anyhow::Result<()> {
    MIGRATOR.run(pool).await?;
    Ok(())
}
