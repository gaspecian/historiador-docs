//! Read-only queries used exclusively by the MCP server to enrich
//! vector search results with page and collection metadata.
//!
//! These queries join `chunks → page_versions → pages → collections`
//! keyed on `(chronik_partition, chronik_offset)` pairs returned by
//! Chronik search hits (ADR-007).

use std::collections::HashMap;

use sqlx::PgPool;
use uuid::Uuid;

/// Enriched metadata for a chunk, used to build MCP query responses.
#[derive(Debug, Clone)]
pub struct EnrichedChunk {
    pub page_version_id: Uuid,
    pub page_id: Uuid,
    pub collection_id: Option<Uuid>,
    pub page_title: String,
    pub language: String,
    pub collection_path: Vec<String>,
    pub heading_path: Vec<String>,
    pub content_markdown: String,
}

/// Look up enrichment for a list of `(partition, offset)` pairs.
/// Returns a map keyed on the same pairs. Pairs without a matching
/// `chunks` row are absent from the map (Chronik may return orphans
/// from prior reindex generations).
pub async fn enrich_chunk_results(
    pool: &PgPool,
    refs: &[(i32, i64)],
) -> anyhow::Result<HashMap<(i32, i64), EnrichedChunk>> {
    if refs.is_empty() {
        return Ok(HashMap::new());
    }

    // Build a VALUES list so the query is one round-trip.
    // Note: "offset" is a SQL reserved word in some dialects; we use the
    // alias `chronik_offset_value` to avoid any parsing ambiguity.
    // RECURSIVE applies to the whole WITH clause so the `coll_path` CTE
    // can self-reference for the collections-tree walk; `refs` is a
    // non-recursive VALUES list and is unaffected.
    let mut sql = String::from(
        "WITH RECURSIVE refs(chronik_partition_value, chronik_offset_value) AS (VALUES ",
    );
    for i in 0..refs.len() {
        if i > 0 {
            sql.push(',');
        }
        sql.push_str(&format!("(${}::int, ${}::bigint)", i * 2 + 1, i * 2 + 2));
    }
    sql.push_str(
        "), \
         coll_path AS ( \
            SELECT id, ARRAY[name]::text[] AS path \
            FROM collections \
            WHERE parent_id IS NULL \
            UNION ALL \
            SELECT c.id, cp.path || c.name \
            FROM collections c \
              JOIN coll_path cp ON c.parent_id = cp.id \
         ) \
         SELECT \
            ch.chronik_partition        AS chronik_partition_value, \
            ch.chronik_offset           AS chronik_offset_value, \
            ch.page_version_id, \
            ch.heading_path, \
            pv.language, \
            pv.title                    AS page_title, \
            p.id                        AS page_id, \
            p.collection_id, \
            COALESCE(cp.path, ARRAY[]::text[]) AS collection_path, \
            pv.content_markdown \
         FROM refs r \
         JOIN chunks ch \
            ON ch.chronik_partition = r.chronik_partition_value \
           AND ch.chronik_offset    = r.chronik_offset_value \
         JOIN page_versions pv ON pv.id = ch.page_version_id \
         JOIN pages         p  ON p.id  = pv.page_id \
         LEFT JOIN coll_path cp ON cp.id = p.collection_id",
    );

    let mut q = sqlx::query(&sql);
    for (p, o) in refs {
        q = q.bind(*p).bind(*o);
    }
    let rows = q.fetch_all(pool).await?;

    use sqlx::Row;
    let mut map = HashMap::with_capacity(rows.len());
    for row in rows {
        let partition: i32 = row.get("chronik_partition_value");
        let offset: i64 = row.get("chronik_offset_value");
        map.insert(
            (partition, offset),
            EnrichedChunk {
                page_version_id: row.get("page_version_id"),
                page_id: row.get("page_id"),
                collection_id: row.try_get("collection_id").ok(),
                page_title: row.get("page_title"),
                language: row.get("language"),
                collection_path: row.get("collection_path"),
                heading_path: row.get("heading_path"),
                content_markdown: row.get("content_markdown"),
            },
        );
    }
    Ok(map)
}
