//! Queries against the `chunks` table.
//!
//! Each row references a Chronik record by its `(chronik_partition,
//! chronik_offset)` pair. Embeddings live in Chronik (ADR-007); only
//! the back-pointer is stored in Postgres so MCP enrichment can map a
//! search hit back to its page/collection.

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
    pub chronik_partition: Option<i32>,
    pub chronik_offset: Option<i64>,
    pub created_at: DateTime<Utc>,
}

pub struct NewChunk {
    pub page_version_id: Uuid,
    pub heading_path: Vec<String>,
    pub section_index: i32,
    pub token_count: i32,
    pub oversized: bool,
    pub language: String,
    pub chronik_partition: i32,
    pub chronik_offset: i64,
}

pub async fn insert_batch(pool: &PgPool, chunks: &[NewChunk]) -> anyhow::Result<Vec<ChunkRow>> {
    let mut rows = Vec::with_capacity(chunks.len());
    for chunk in chunks {
        let row = sqlx::query_as::<_, ChunkRow>(
            "INSERT INTO chunks \
               (page_version_id, heading_path, section_index, token_count, \
                oversized, language, chronik_partition, chronik_offset) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
             RETURNING *",
        )
        .bind(chunk.page_version_id)
        .bind(&chunk.heading_path)
        .bind(chunk.section_index)
        .bind(chunk.token_count)
        .bind(chunk.oversized)
        .bind(&chunk.language)
        .bind(chunk.chronik_partition)
        .bind(chunk.chronik_offset)
        .fetch_one(pool)
        .await?;
        rows.push(row);
    }
    Ok(rows)
}

pub async fn delete_by_page_version(pool: &PgPool, page_version_id: Uuid) -> anyhow::Result<u64> {
    let result = sqlx::query("DELETE FROM chunks WHERE page_version_id = $1")
        .bind(page_version_id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

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
