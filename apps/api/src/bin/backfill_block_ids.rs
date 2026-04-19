//! One-shot backfill: parse every `page_versions.content_markdown`
//! row, re-serialize it through `historiador_blocks`, and write back.
//! Idempotent — rows that already carry `<!-- block:<uuid> -->`
//! comments are untouched.
//!
//! Usage:
//!   cargo run -p historiador_api --bin backfill-block-ids
//!
//! Environment:
//!   DATABASE_URL_READWRITE  required (same value as the API binary)
//!   EDITOR_V2_ENABLED       must be true/1/yes/on — safety interlock
//!                            so an accidental production run on a
//!                            flag-off environment is impossible
//!
//! Rationale: ADR-010 requires stable block IDs for diff and comment
//! anchoring. Pages created before Sprint 11 carry plain markdown
//! without IDs; this script adds them once, in place, so the editor
//! v2 WebSocket can negotiate block-op acks against existing content.

use anyhow::Context;
use historiador_blocks::{parse_markdown, serialize_markdown};
use sqlx::{postgres::PgPoolOptions, PgPool};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    tracing_subscriber::fmt().with_target(false).init();

    let flag = std::env::var("EDITOR_V2_ENABLED").unwrap_or_default();
    if !matches!(
        flag.to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    ) {
        anyhow::bail!(
            "EDITOR_V2_ENABLED is not true. Refusing to run the backfill on a flag-off \
             environment. Set EDITOR_V2_ENABLED=true and re-run if this is intentional."
        );
    }

    let database_url =
        std::env::var("DATABASE_URL_READWRITE").context("DATABASE_URL_READWRITE is required")?;
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&database_url)
        .await
        .context("failed to connect to postgres")?;

    let stats = backfill(&pool).await?;
    tracing::info!(
        scanned = stats.scanned,
        skipped_already_tagged = stats.skipped,
        updated = stats.updated,
        failed = stats.failed,
        "backfill complete"
    );
    Ok(())
}

#[derive(Debug, Default)]
struct Stats {
    scanned: usize,
    skipped: usize,
    updated: usize,
    failed: usize,
}

async fn backfill(pool: &PgPool) -> anyhow::Result<Stats> {
    let rows: Vec<(uuid::Uuid, String)> =
        sqlx::query_as("SELECT id, content_markdown FROM page_versions")
            .fetch_all(pool)
            .await?;

    let mut stats = Stats::default();
    for (id, markdown) in rows {
        stats.scanned += 1;

        if markdown.contains("<!-- block:") {
            stats.skipped += 1;
            continue;
        }

        let tree = match parse_markdown(&markdown) {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!(page_version_id = %id, error = %e, "parse failed — skipping");
                stats.failed += 1;
                continue;
            }
        };

        let tagged = serialize_markdown(&tree);
        if tagged == markdown {
            // Empty or otherwise unchanged — still count as skipped.
            stats.skipped += 1;
            continue;
        }

        let res = sqlx::query("UPDATE page_versions SET content_markdown = $1 WHERE id = $2")
            .bind(&tagged)
            .bind(id)
            .execute(pool)
            .await;

        match res {
            Ok(_) => stats.updated += 1,
            Err(e) => {
                tracing::warn!(page_version_id = %id, error = %e, "update failed");
                stats.failed += 1;
            }
        }
    }

    Ok(stats)
}
