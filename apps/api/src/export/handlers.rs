//! Markdown export handlers — thin wrappers over
//! [`crate::application::export`]. The use cases return raw data;
//! zip streaming and filename formatting stay here because they are
//! wire-format concerns.

use std::sync::Arc;

use async_zip::tokio::write::ZipFileWriter;
use async_zip::{Compression, ZipEntryBuilder};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::Utc;
use serde::Deserialize;
use tokio_util::io::ReaderStream;
use uuid::Uuid;

use crate::auth::extractor::AuthUser;
use crate::domain::port::export_repository::PublishedPageExport;
use crate::domain::value::Language;
use crate::error::ApiError;
use crate::state::AppState;

// ---- wire-format helpers ----

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

fn build_zip_entry_name(row: &PublishedPageExport) -> String {
    let slug = sanitize_path_segment(&row.page_slug);
    let lang = sanitize_path_segment(row.language.as_str());
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

fn render_front_matter(row: &PublishedPageExport) -> String {
    fn yaml_string(s: &str) -> String {
        let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
        format!("\"{escaped}\"")
    }

    let title = yaml_string(&row.title);
    let lang = yaml_string(row.language.as_str());
    let path = yaml_string(row.collection_path.as_deref().unwrap_or(""));
    let author = yaml_string(row.author_email.as_deref().unwrap_or("unknown"));
    let updated = row.updated_at.to_rfc3339();

    format!(
        "---\ntitle: {title}\ncollection_path: {path}\nlanguage: {lang}\nlast_updated: {updated}\nauthor: {author}\n---\n\n"
    )
}

// ---- zip of the whole workspace ----

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
    let view = state
        .use_cases
        .export_workspace
        .execute(auth.as_actor())
        .await?;
    let workspace_name = view.workspace.name.clone();
    let rows = view.pages;

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
            if let Err(e) = writer.write_entry_whole(entry, contents.as_bytes()).await {
                tracing::error!(entry = %entry_name, error = %e, "zip entry write failed");
                return;
            }
        }
        if let Err(e) = writer.close().await {
            tracing::error!(error = %e, "zip close failed");
        }
    });

    let today = Utc::now().format("%Y%m%d");
    let filename = format!(
        "{}-{}.zip",
        sanitize_path_segment(&workspace_name)
            .replace(' ', "-")
            .to_lowercase(),
        today
    );
    let disposition = format!("attachment; filename=\"{filename}\"");

    let body = Body::from_stream(ReaderStream::new(read_half));
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/zip")
        .header(header::CONTENT_DISPOSITION, disposition)
        .body(body)
        .map_err(|e| ApiError::Internal(e.into()))
}

// ---- single page export ----

#[derive(Debug, Deserialize)]
pub struct PageExportQuery {
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
    let row = state
        .use_cases
        .export_page
        .execute(
            auth.as_actor(),
            page_id,
            q.language.map(Language::from_trusted),
        )
        .await?;

    let mut body = render_front_matter(&row);
    body.push_str(&row.content_markdown);
    if !body.ends_with('\n') {
        body.push('\n');
    }

    let filename = format!(
        "{}.{}.md",
        sanitize_path_segment(&row.page_slug),
        sanitize_path_segment(row.language.as_str())
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

    fn row(slug: &str, path: Option<&str>, lang: &str) -> PublishedPageExport {
        PublishedPageExport {
            page_id: Uuid::nil(),
            page_slug: slug.to_string(),
            collection_path: path.map(String::from),
            language: Language::from_trusted(lang),
            title: "t".to_string(),
            content_markdown: "c".to_string(),
            author_email: None,
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn sanitize_strips_path_separators() {
        assert_eq!(sanitize_path_segment("a/b"), "a_b");
        assert_eq!(sanitize_path_segment("weird:name*?"), "weird_name__");
        assert_eq!(sanitize_path_segment("  "), "untitled");
        assert_eq!(sanitize_path_segment("normal"), "normal");
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
