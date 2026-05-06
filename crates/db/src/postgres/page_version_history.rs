//! Queries against the `page_version_history` table.
//!
//! This table is an append-only audit trail. Every save and publish
//! snapshots the current content. Rows are never updated or deleted
//! (except via CASCADE when the parent page is deleted).

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

/// Full row from the `page_version_history` table.
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct PageVersionHistoryRow {
    pub id: Uuid,
    pub page_id: Uuid,
    pub language: String,
    pub title: String,
    pub content_markdown: String,
    pub is_published: bool,
    pub author_id: Option<Uuid>,
    pub version_number: i32,
    pub created_at: DateTime<Utc>,
}

/// Summary row for listing (no full content, includes preview).
#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct PageVersionHistorySummary {
    pub id: Uuid,
    pub version_number: i32,
    pub title: String,
    pub content_preview: String,
    pub is_published: bool,
    pub author_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

/// Insert a new immutable snapshot. The `version_number` is auto-
/// incremented per (page_id, language) using a CTE to avoid races.
pub async fn insert(
    pool: &PgPool,
    page_id: Uuid,
    language: &str,
    title: &str,
    content_markdown: &str,
    is_published: bool,
    author_id: Option<Uuid>,
) -> anyhow::Result<PageVersionHistoryRow> {
    let row = sqlx::query_as::<_, PageVersionHistoryRow>(
        "WITH next_num AS (
             SELECT COALESCE(MAX(version_number), 0) + 1 AS vn
             FROM page_version_history
             WHERE page_id = $1 AND language = $2
         )
         INSERT INTO page_version_history
             (page_id, language, title, content_markdown, is_published, author_id, version_number)
         SELECT $1, $2, $3, $4, $5, $6, vn FROM next_num
         RETURNING *",
    )
    .bind(page_id)
    .bind(language)
    .bind(title)
    .bind(content_markdown)
    .bind(is_published)
    .bind(author_id)
    .fetch_one(pool)
    .await?;

    Ok(row)
}

/// Paginated list of version history for a page + language, newest first.
/// Returns (summaries, total_count).
pub async fn list_by_page_and_language(
    pool: &PgPool,
    page_id: Uuid,
    language: &str,
    page: i64,
    per_page: i64,
) -> anyhow::Result<(Vec<PageVersionHistorySummary>, i64)> {
    let offset = (page - 1) * per_page;

    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM page_version_history \
         WHERE page_id = $1 AND language = $2",
    )
    .bind(page_id)
    .bind(language)
    .fetch_one(pool)
    .await?;

    let rows = sqlx::query_as::<_, PageVersionHistorySummary>(
        "SELECT id, version_number, title, \
                LEFT(content_markdown, 200) AS content_preview, \
                is_published, author_id, created_at \
         FROM page_version_history \
         WHERE page_id = $1 AND language = $2 \
         ORDER BY created_at DESC \
         LIMIT $3 OFFSET $4",
    )
    .bind(page_id)
    .bind(language)
    .bind(per_page)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok((rows, total.0))
}

/// Fetch a single version history entry by ID.
pub async fn find_by_id(pool: &PgPool, id: Uuid) -> anyhow::Result<Option<PageVersionHistoryRow>> {
    let row = sqlx::query_as::<_, PageVersionHistoryRow>(
        "SELECT * FROM page_version_history WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row)
}

/// Check whether a recent snapshot exists (within `seconds` ago) for
/// a given page + language. Used to debounce auto-save snapshots.
pub async fn has_recent_snapshot(
    pool: &PgPool,
    page_id: Uuid,
    language: &str,
    seconds: i32,
) -> anyhow::Result<bool> {
    let exists: (bool,) = sqlx::query_as(
        "SELECT EXISTS(
             SELECT 1 FROM page_version_history
             WHERE page_id = $1
               AND language = $2
               AND created_at > now() - make_interval(secs => $3::double precision)
         )",
    )
    .bind(page_id)
    .bind(language)
    .bind(seconds as f64)
    .fetch_one(pool)
    .await?;

    Ok(exists.0)
}
