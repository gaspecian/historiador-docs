//! Read-only queries used exclusively by the MCP server to enrich
//! vector search results with page and collection metadata.
//!
//! These queries join `page_versions → pages → collections` and use a
//! recursive CTE to build the full collection path (e.g.,
//! `["Engineering", "APIs", "Authentication"]`).

use std::collections::HashMap;

use sqlx::PgPool;
use uuid::Uuid;

/// Enriched metadata for a chunk, used to build MCP query responses.
#[derive(Debug, Clone)]
pub struct EnrichedChunkMeta {
    pub page_version_id: Uuid,
    pub page_title: String,
    pub language: String,
    pub page_id: Uuid,
    pub collection_id: Option<Uuid>,
    pub collection_path: Vec<String>,
}

/// Internal row type for the enrichment query. The recursive CTE returns
/// `collection_path` as a Postgres `text[]` which sqlx decodes as `Vec<String>`.
#[derive(Debug, sqlx::FromRow)]
struct EnrichmentRow {
    page_version_id: Uuid,
    page_title: String,
    language: String,
    page_id: Uuid,
    collection_id: Option<Uuid>,
    collection_path: Vec<String>,
}

/// Given a set of page_version_ids (from vector search results), return
/// enriched metadata for each one — page title, language, and the full
/// collection ancestry path.
///
/// Returns a `HashMap` keyed by `page_version_id` for O(1) lookup when
/// merging with `ChunkRef` results.
pub async fn enrich_chunk_results(
    pool: &PgPool,
    page_version_ids: &[Uuid],
) -> anyhow::Result<HashMap<Uuid, EnrichedChunkMeta>> {
    if page_version_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let rows = sqlx::query_as::<_, EnrichmentRow>(
        "WITH RECURSIVE collection_tree AS ( \
             SELECT id, parent_id, name, ARRAY[name]::text[] AS path \
             FROM collections \
             WHERE parent_id IS NULL \
           UNION ALL \
             SELECT c.id, c.parent_id, c.name, ct.path || c.name \
             FROM collections c \
             JOIN collection_tree ct ON c.parent_id = ct.id \
         ) \
         SELECT \
             pv.id AS page_version_id, \
             pv.title AS page_title, \
             pv.language, \
             p.id AS page_id, \
             p.collection_id, \
             COALESCE(ct.path, ARRAY[]::text[]) AS collection_path \
         FROM page_versions pv \
         JOIN pages p ON pv.page_id = p.id \
         LEFT JOIN collection_tree ct ON p.collection_id = ct.id \
         WHERE pv.id = ANY($1)",
    )
    .bind(page_version_ids)
    .fetch_all(pool)
    .await?;

    let map = rows
        .into_iter()
        .map(|r| {
            (
                r.page_version_id,
                EnrichedChunkMeta {
                    page_version_id: r.page_version_id,
                    page_title: r.page_title,
                    language: r.language,
                    page_id: r.page_id,
                    collection_id: r.collection_id,
                    collection_path: r.collection_path,
                },
            )
        })
        .collect();

    Ok(map)
}
