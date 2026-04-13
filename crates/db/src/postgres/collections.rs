//! Queries against the `collections` table.
//!
//! Collections use an adjacency list model (parent_id) for unbounded
//! nesting. The ON DELETE CASCADE constraint in the schema handles
//! recursive deletion of children and associated pages automatically.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize, utoipa::ToSchema)]
pub struct Collection {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub name: String,
    pub slug: String,
    pub sort_order: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Insert a new collection, returning the created row.
pub async fn insert(
    pool: &PgPool,
    workspace_id: Uuid,
    parent_id: Option<Uuid>,
    name: &str,
    slug: &str,
) -> anyhow::Result<Collection> {
    let row = sqlx::query_as::<_, Collection>(
        "INSERT INTO collections (workspace_id, parent_id, name, slug) \
         VALUES ($1, $2, $3, $4) \
         RETURNING *",
    )
    .bind(workspace_id)
    .bind(parent_id)
    .bind(name)
    .bind(slug)
    .fetch_one(pool)
    .await?;
    Ok(row)
}

/// Find a collection by id, scoped to a workspace.
pub async fn find_by_id(
    pool: &PgPool,
    id: Uuid,
    workspace_id: Uuid,
) -> anyhow::Result<Option<Collection>> {
    let row = sqlx::query_as::<_, Collection>(
        "SELECT * FROM collections WHERE id = $1 AND workspace_id = $2",
    )
    .bind(id)
    .bind(workspace_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// List all collections for a workspace, ordered by sort_order then name.
pub async fn list_by_workspace(
    pool: &PgPool,
    workspace_id: Uuid,
) -> anyhow::Result<Vec<Collection>> {
    let rows = sqlx::query_as::<_, Collection>(
        "SELECT * FROM collections WHERE workspace_id = $1 \
         ORDER BY sort_order, name",
    )
    .bind(workspace_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Update a collection's name and/or parent. Returns the updated row,
/// or `None` if the collection was not found in the given workspace.
pub async fn update(
    pool: &PgPool,
    id: Uuid,
    workspace_id: Uuid,
    name: Option<&str>,
    slug: Option<&str>,
    parent_id: Option<Option<Uuid>>,
) -> anyhow::Result<Option<Collection>> {
    // Build the SET clause dynamically based on which fields are provided.
    // This avoids overwriting fields the caller didn't intend to change.
    let mut set_clauses = Vec::new();
    let mut param_index = 3u32; // $1 = id, $2 = workspace_id

    if name.is_some() {
        set_clauses.push(format!("name = ${param_index}"));
        param_index += 1;
    }
    if slug.is_some() {
        set_clauses.push(format!("slug = ${param_index}"));
        param_index += 1;
    }
    if parent_id.is_some() {
        set_clauses.push(format!("parent_id = ${param_index}"));
        // param_index += 1; // last param
    }

    if set_clauses.is_empty() {
        return find_by_id(pool, id, workspace_id).await;
    }

    let sql = format!(
        "UPDATE collections SET {} WHERE id = $1 AND workspace_id = $2 RETURNING *",
        set_clauses.join(", ")
    );

    let mut query = sqlx::query_as::<_, Collection>(&sql)
        .bind(id)
        .bind(workspace_id);

    if let Some(n) = name {
        query = query.bind(n);
    }
    if let Some(s) = slug {
        query = query.bind(s);
    }
    if let Some(pid) = parent_id {
        query = query.bind(pid);
    }

    let row = query.fetch_optional(pool).await?;
    Ok(row)
}

/// Delete a collection and all its children/pages (via ON DELETE CASCADE).
/// Returns the number of deleted rows (1 if found, 0 if not).
pub async fn delete_cascade(pool: &PgPool, id: Uuid, workspace_id: Uuid) -> anyhow::Result<u64> {
    let result = sqlx::query("DELETE FROM collections WHERE id = $1 AND workspace_id = $2")
        .bind(id)
        .bind(workspace_id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}
