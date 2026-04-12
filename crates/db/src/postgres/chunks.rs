//! Queries against the `chunks` table.
//!
//! Chunk rows in Postgres hold metadata and a `vexfs_ref` pointer to
//! the embedding in VexFS (ADR-001). Embeddings are never stored in
//! Postgres — only the opaque reference.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct ChunkRow {
    pub id: Uuid,
    pub page_version_id: Uuid,
    pub heading_path: Vec<String>,
    pub section_index: i32,
    pub token_count: i32,
    pub oversized: bool,
    pub language: String,
    pub vexfs_ref: String,
    pub created_at: DateTime<Utc>,
}

/// Input struct for inserting a new chunk.
pub struct NewChunk {
    pub page_version_id: Uuid,
    pub heading_path: Vec<String>,
    pub section_index: i32,
    pub token_count: i32,
    pub oversized: bool,
    pub language: String,
    pub vexfs_ref: String,
}

/// Insert a batch of chunks. Returns the inserted rows.
pub async fn insert_batch(
    pool: &PgPool,
    chunks: &[NewChunk],
) -> anyhow::Result<Vec<ChunkRow>> {
    let mut rows = Vec::with_capacity(chunks.len());
    for chunk in chunks {
        let row = sqlx::query_as::<_, ChunkRow>(
            "INSERT INTO chunks \
               (page_version_id, heading_path, section_index, token_count, \
                oversized, language, vexfs_ref) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) \
             RETURNING *",
        )
        .bind(chunk.page_version_id)
        .bind(&chunk.heading_path)
        .bind(chunk.section_index)
        .bind(chunk.token_count)
        .bind(chunk.oversized)
        .bind(&chunk.language)
        .bind(&chunk.vexfs_ref)
        .fetch_one(pool)
        .await?;
        rows.push(row);
    }
    Ok(rows)
}

/// Delete all chunks for a page version. Returns the number of deleted rows.
pub async fn delete_by_page_version(
    pool: &PgPool,
    page_version_id: Uuid,
) -> anyhow::Result<u64> {
    let result =
        sqlx::query("DELETE FROM chunks WHERE page_version_id = $1")
            .bind(page_version_id)
            .execute(pool)
            .await?;
    Ok(result.rows_affected())
}

/// Fetch all chunks for a page version, ordered by section_index.
pub async fn find_by_page_version(
    pool: &PgPool,
    page_version_id: Uuid,
) -> anyhow::Result<Vec<ChunkRow>> {
    let rows = sqlx::query_as::<_, ChunkRow>(
        "SELECT * FROM chunks WHERE page_version_id = $1 ORDER BY section_index",
    )
    .bind(page_version_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
