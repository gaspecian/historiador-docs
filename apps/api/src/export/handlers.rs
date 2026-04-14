//! Markdown export handlers.
//!
//! - `GET /export` — admin-only zip of the full workspace.
//! - `GET /pages/:id/export` — one markdown file per language version.

use std::sync::Arc;

use async_zip::tokio::write::ZipFileWriter;
use async_zip::{Compression, ZipEntryBuilder};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use historiador_db::postgres::{pages::PageStatus, users::Role, workspaces};
use serde::Deserialize;
use sqlx::FromRow;
use tokio_util::io::ReaderStream;
use uuid::Uuid;

use crate::auth::{extractor::AuthUser, rbac::require_role};
use crate::error::ApiError;
use crate::state::AppState;

// ---- zip of the whole workspace ----

/// Row returned by the recursive-CTE walk of collections + published
/// page versions. `collection_path` is the slash-joined collection
/// name chain; `None` for pages not nested under a collection.
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

async fn fetch_published_rows(
    pool: &sqlx::PgPool,
    workspace_id: Uuid,
) -> Result<Vec<PublishedRow>, sqlx::Error> {
    sqlx::query_as::<_, PublishedRow>(
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
    .fetch_all(pool)
    .await
}

/// Strip filesystem-unsafe characters so the zip entry name is a
/// legal path on Windows, macOS and Linux.
fn sanitize_path_segment(input: &str) -> String {
    let trimmed: String = input
        .chars()
        .map(|c| {
            if c.is_control() {
                '_'
            } else {
                match c {
                    '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
                    other => other,
                }
            }
        })
        .collect();
    let trimmed = trimmed.trim().trim_matches('.');
    if trimmed.is_empty() {
        "untitled".to_string()
    } else {
        trimmed.to_string()
    }
}

fn build_zip_entry_name(row: &PublishedRow) -> String {
    let slug = sanitize_path_segment(&row.page_slug);
    let lang = sanitize_path_segment(&row.language);
    let filename = format!("{slug}.{lang}.md");
    match &row.collection_path {
        Some(path) => {
            let sanitized = path
                .split('/')
                .map(sanitize_path_segment)
                .collect::<Vec<_>>()
                .join("/");
            format!("{sanitized}/{filename}")
        }
        None => format!("_uncategorized/{filename}"),
    }
}

fn render_front_matter(row: &PublishedRow) -> String {
    // Minimal, ambiguity-free YAML. Titles are wrapped in double
    // quotes with internal quotes escaped — mirror what comrak-style
    // tooling expects. No block scalars: keeps the escaping local.
    fn yaml_string(s: &str) -> String {
        let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
        format!("\"{escaped}\"")
    }

    let title = yaml_string(&row.title);
    let lang = yaml_string(&row.language);
    let path = yaml_string(row.collection_path.as_deref().unwrap_or(""));
    let author = yaml_string(row.author_email.as_deref().unwrap_or("unknown"));
    let updated = row.updated_at.to_rfc3339();

    format!(
        "---\ntitle: {title}\ncollection_path: {path}\nlanguage: {lang}\nlast_updated: {updated}\nauthor: {author}\n---\n\n"
    )
}

#[utoipa::path(
    get,
    path = "/export",
    responses(
        (status = 200, description = "zip of all published pages", content_type = "application/zip"),
        (status = 401, description = "unauthenticated"),
        (status = 403, description = "caller is not admin"),
    ),
    security(("bearer" = [])),
    tag = "export"
)]
pub async fn export_workspace(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> Result<Response, ApiError> {
    require_role(&auth, Role::Admin)?;

    let ws = workspaces::find_by_id(&state.pool, auth.workspace_id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;

    let rows = fetch_published_rows(&state.pool, auth.workspace_id)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;

    // DuplexStream: one side is written to by the zip encoder, the
    // other streams out as the HTTP body. No full-zip buffering.
    let (write_half, read_half) = tokio::io::duplex(64 * 1024);

    tokio::spawn(async move {
        let mut writer = ZipFileWriter::with_tokio(write_half);
        for row in rows {
            let entry_name = build_zip_entry_name(&row);
            let entry = ZipEntryBuilder::new(entry_name.clone().into(), Compression::Deflate);
            let mut contents = render_front_matter(&row);
            contents.push_str(&row.content_markdown);
            if !contents.ends_with('\n') {
                contents.push('\n');
            }
            match writer.write_entry_whole(entry, contents.as_bytes()).await {
                Ok(()) => {}
                Err(e) => {
                    tracing::error!(entry = %entry_name, error = %e, "zip entry write failed");
                    return;
                }
            }
        }
        if let Err(e) = writer.close().await {
            tracing::error!(error = %e, "zip close failed");
        }
    });

    let today = Utc::now().format("%Y%m%d");
    let filename = format!(
        "{}-{}.zip",
        sanitize_path_segment(&ws.name)
            .replace(' ', "-")
            .to_lowercase(),
        today
    );
    let disposition = format!("attachment; filename=\"{filename}\"");

    let body = Body::from_stream(ReaderStream::new(read_half));
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/zip")
        .header(header::CONTENT_DISPOSITION, disposition)
        .body(body)
        .map_err(|e| ApiError::Internal(e.into()))?;
    Ok(response)
}

// ---- single page export ----

#[derive(Debug, Deserialize)]
pub struct PageExportQuery {
    /// Optional BCP 47 language tag. Defaults to the workspace primary
    /// language when omitted.
    pub language: Option<String>,
}

#[utoipa::path(
    get,
    path = "/pages/{id}/export",
    params(
        ("id" = Uuid, Path, description = "Page id"),
        ("language" = Option<String>, Query, description = "BCP 47 language tag; defaults to workspace primary"),
    ),
    responses(
        (status = 200, description = "markdown with YAML front-matter", content_type = "text/markdown"),
        (status = 401, description = "unauthenticated"),
        (status = 404, description = "page or version not found"),
    ),
    security(("bearer" = [])),
    tag = "export"
)]
pub async fn export_page(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(page_id): Path<Uuid>,
    axum::extract::Query(q): axum::extract::Query<PageExportQuery>,
) -> Result<Response, ApiError> {
    require_role(&auth, Role::Author)?;

    let ws = workspaces::find_by_id(&state.pool, auth.workspace_id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;

    let language = q.language.unwrap_or_else(|| ws.primary_language.clone());

    // Reuse the same walk so authorization + collection_path come
    // from one authoritative place — filtering in Rust keeps the
    // handler simple and saves us a second SQL query for one page.
    let rows = fetch_published_rows(&state.pool, auth.workspace_id)
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;

    let row = rows
        .into_iter()
        .find(|r| r.page_id == page_id && r.language == language)
        .ok_or(ApiError::NotFound)?;

    let mut body = render_front_matter(&row);
    body.push_str(&row.content_markdown);
    if !body.ends_with('\n') {
        body.push('\n');
    }

    let filename = format!(
        "{}.{}.md",
        sanitize_path_segment(&row.page_slug),
        sanitize_path_segment(&row.language)
    );
    let disposition = format!("attachment; filename=\"{filename}\"");

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/markdown; charset=utf-8"),
            (header::CONTENT_DISPOSITION, disposition.as_str()),
        ],
        body,
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_strips_path_separators() {
        assert_eq!(sanitize_path_segment("a/b"), "a_b");
        assert_eq!(sanitize_path_segment("weird:name*?"), "weird_name__");
        assert_eq!(sanitize_path_segment("  "), "untitled");
        assert_eq!(sanitize_path_segment("normal"), "normal");
    }

    fn row(slug: &str, path: Option<&str>, lang: &str) -> PublishedRow {
        PublishedRow {
            page_id: Uuid::nil(),
            page_slug: slug.to_string(),
            collection_path: path.map(String::from),
            language: lang.to_string(),
            title: "t".to_string(),
            content_markdown: "c".to_string(),
            author_email: None,
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn zip_entry_name_uses_sanitized_collection_path() {
        let r = row("apis", Some("Engineering/Backend"), "en");
        assert_eq!(build_zip_entry_name(&r), "Engineering/Backend/apis.en.md");
    }

    #[test]
    fn zip_entry_name_for_uncategorized_page() {
        let r = row("index", None, "en");
        assert_eq!(build_zip_entry_name(&r), "_uncategorized/index.en.md");
    }

    #[test]
    fn yaml_front_matter_escapes_quotes() {
        let mut r = row("slug", None, "en");
        r.title = "She said \"hi\"".to_string();
        let y = render_front_matter(&r);
        assert!(y.contains("title: \"She said \\\"hi\\\"\""));
    }
}
