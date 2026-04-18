//! Postgres adapter for `ExportRepository`. Owns the recursive-CTE
//! walk that joins pages → collections → users to produce export rows
//! with a full collection path and author email.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use historiador_db::postgres::pages::PageStatus;

use crate::domain::error::ApplicationError;
use crate::domain::port::export_repository::{ExportRepository, PublishedPageExport};
use crate::domain::value::Language;

#[derive(Debug, FromRow)]
struct PublishedRow {
    page_id: Uuid,
    page_slug: String,
    collection_path: Option<String>,
    language: String,
    title: String,
    content_markdown: String,
    author_email: Option<String>,
    updated_at: DateTime<Utc>,
}

pub struct PostgresExportRepository {
    pool: PgPool,
}

impl PostgresExportRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ExportRepository for PostgresExportRepository {
    async fn find_all_published(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<PublishedPageExport>, ApplicationError> {
        let rows = sqlx::query_as::<_, PublishedRow>(
            r#"
            WITH RECURSIVE tree AS (
                SELECT id, name, parent_id, name AS path
                  FROM collections
                 WHERE workspace_id = $1 AND parent_id IS NULL
              UNION ALL
                SELECT c.id, c.name, c.parent_id, t.path || '/' || c.name
                  FROM collections c
                  JOIN tree t ON c.parent_id = t.id
                 WHERE c.workspace_id = $1
            )
            SELECT
                p.id          AS page_id,
                p.slug        AS page_slug,
                t.path        AS collection_path,
                pv.language   AS language,
                pv.title      AS title,
                pv.content_markdown AS content_markdown,
                u.email::TEXT AS author_email,
                pv.updated_at AS updated_at
              FROM page_versions pv
              JOIN pages p ON p.id = pv.page_id
         LEFT JOIN tree  t ON t.id = p.collection_id
         LEFT JOIN users u ON u.id = pv.author_id
             WHERE p.workspace_id = $1
               AND pv.status = $2
          ORDER BY t.path NULLS FIRST, p.slug, pv.language
            "#,
        )
        .bind(workspace_id)
        .bind(PageStatus::Published)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ApplicationError::Infrastructure(e.into()))?;

        Ok(rows.into_iter().map(map).collect())
    }
}

fn map(row: PublishedRow) -> PublishedPageExport {
    PublishedPageExport {
        page_id: row.page_id,
        page_slug: row.page_slug,
        collection_path: row.collection_path,
        language: Language::from_trusted(row.language),
        title: row.title,
        content_markdown: row.content_markdown,
        author_email: row.author_email,
        updated_at: row.updated_at,
    }
}
