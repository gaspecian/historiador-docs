//! Queries against the `page_versions` table.
//!
//! Each page has one version per language (BCP 47 tag). The UNIQUE
//! constraint on `(page_id, language)` means upsert semantics are
//! natural for creating-or-updating a draft.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use super::pages::PageStatus;

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize, utoipa::ToSchema)]
pub struct PageVersion {
    pub id: Uuid,
    pub page_id: Uuid,
    pub language: String,
    pub title: String,
    pub content_markdown: String,
    pub status: PageStatus,
    pub author_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Insert or update a page version for a given language. On conflict
/// (same page_id + language), updates the content and title.
pub async fn upsert(
    pool: &PgPool,
    page_id: Uuid,
    language: &str,
    title: &str,
    content_markdown: &str,
    author_id: Uuid,
    status: PageStatus,
) -> anyhow::Result<PageVersion> {
    let row = sqlx::query_as::<_, PageVersion>(
        "INSERT INTO page_versions \
           (page_id, language, title, content_markdown, author_id, status) \
         VALUES ($1, $2, $3, $4, $5, $6) \
         ON CONFLICT (page_id, language) DO UPDATE SET \
           title = EXCLUDED.title, \
           content_markdown = EXCLUDED.content_markdown, \
           author_id = EXCLUDED.author_id, \
           status = EXCLUDED.status \
         RETURNING *",
    )
    .bind(page_id)
    .bind(language)
    .bind(title)
    .bind(content_markdown)
    .bind(author_id)
    .bind(status)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// Fetch all versions for a page (one per language).
pub async fn find_by_page(pool: &PgPool, page_id: Uuid) -> anyhow::Result<Vec<PageVersion>> {
    let rows = sqlx::query_as::<_, PageVersion>(
        "SELECT * FROM page_versions WHERE page_id = $1 ORDER BY language",
    )
    .bind(page_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Fetch a specific language version of a page.
pub async fn find_by_page_and_language(
    pool: &PgPool,
    page_id: Uuid,
    language: &str,
) -> anyhow::Result<Option<PageVersion>> {
    let row = sqlx::query_as::<_, PageVersion>(
        "SELECT * FROM page_versions \
         WHERE page_id = $1 AND language = $2",
    )
    .bind(page_id)
    .bind(language)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Update the status of all versions of a page at once. Returns the
/// number of rows affected.
pub async fn update_status_all(
    pool: &PgPool,
    page_id: Uuid,
    status: PageStatus,
) -> anyhow::Result<u64> {
    let result = sqlx::query(
        "UPDATE page_versions SET status = $2 WHERE page_id = $1",
    )
    .bind(page_id)
    .bind(status)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}
