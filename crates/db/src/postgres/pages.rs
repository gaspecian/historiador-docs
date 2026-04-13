//! Queries against the `pages` table.
//!
//! A page is the top-level entity for documentation content. Each page
//! belongs to a workspace and optionally a collection. The actual
//! content lives in `page_versions` (one per language).

use chrono::{DateTime, Utc};
use sqlx::PgPool;
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
#[sqlx(type_name = "page_status", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum PageStatus {
    Draft,
    Published,
}

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize, utoipa::ToSchema)]
pub struct Page {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub collection_id: Option<Uuid>,
    pub slug: String,
    pub status: PageStatus,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Insert a new page, returning the created row.
pub async fn insert(
    pool: &PgPool,
    workspace_id: Uuid,
    collection_id: Option<Uuid>,
    slug: &str,
    created_by: Uuid,
) -> anyhow::Result<Page> {
    let row = sqlx::query_as::<_, Page>(
        "INSERT INTO pages (workspace_id, collection_id, slug, created_by) \
         VALUES ($1, $2, $3, $4) \
         RETURNING *",
    )
    .bind(workspace_id)
    .bind(collection_id)
    .bind(slug)
    .bind(created_by)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// Find a page by id, scoped to a workspace.
pub async fn find_by_id(
    pool: &PgPool,
    id: Uuid,
    workspace_id: Uuid,
) -> anyhow::Result<Option<Page>> {
    let row = sqlx::query_as::<_, Page>("SELECT * FROM pages WHERE id = $1 AND workspace_id = $2")
        .bind(id)
        .bind(workspace_id)
        .fetch_optional(pool)
        .await?;
    Ok(row)
}

/// Update the page-level status. Returns the updated row or `None`
/// if the page was not found.
pub async fn update_status(
    pool: &PgPool,
    id: Uuid,
    workspace_id: Uuid,
    status: PageStatus,
) -> anyhow::Result<Option<Page>> {
    let row = sqlx::query_as::<_, Page>(
        "UPDATE pages SET status = $3 \
         WHERE id = $1 AND workspace_id = $2 \
         RETURNING *",
    )
    .bind(id)
    .bind(workspace_id)
    .bind(status)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Search pages by title (ILIKE on `page_versions.title`), scoped to a
/// workspace. Returns matching pages ordered by relevance (title match).
pub async fn search(pool: &PgPool, workspace_id: Uuid, query: &str) -> anyhow::Result<Vec<Page>> {
    let pattern = format!("%{query}%");
    let rows = sqlx::query_as::<_, Page>(
        "SELECT DISTINCT p.* FROM pages p \
         JOIN page_versions pv ON pv.page_id = p.id \
         WHERE p.workspace_id = $1 AND pv.title ILIKE $2 \
         ORDER BY p.updated_at DESC",
    )
    .bind(workspace_id)
    .bind(&pattern)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// List pages in a collection (or at the workspace root if collection_id
/// is None), ordered by creation date.
pub async fn list_by_collection(
    pool: &PgPool,
    workspace_id: Uuid,
    collection_id: Option<Uuid>,
) -> anyhow::Result<Vec<Page>> {
    let rows = match collection_id {
        Some(cid) => {
            sqlx::query_as::<_, Page>(
                "SELECT * FROM pages \
                 WHERE workspace_id = $1 AND collection_id = $2 \
                 ORDER BY created_at",
            )
            .bind(workspace_id)
            .bind(cid)
            .fetch_all(pool)
            .await?
        }
        None => {
            sqlx::query_as::<_, Page>(
                "SELECT * FROM pages \
                 WHERE workspace_id = $1 AND collection_id IS NULL \
                 ORDER BY created_at",
            )
            .bind(workspace_id)
            .fetch_all(pool)
            .await?
        }
    };
    Ok(rows)
}
