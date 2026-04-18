//! `/pages` HTTP handlers — thin Clean Architecture wrappers over
//! [`crate::application::pages`]. All business logic (role checks,
//! snapshots, event production, async chunking) lives in use cases.

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use historiador_db::postgres::pages::PageStatus as DbPageStatus;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::application::pages::{
    CreatePageCommand, ListVersionHistoryCommand, PageView, PageVersionsView, UpdatePageCommand,
    VersionHistoryPage,
};
use crate::presentation::extractor::AuthUser;
use crate::domain::entity::{Page, PageVersion, VersionHistoryEntry};
use crate::domain::value::{Language, PageStatus};
use crate::presentation::error::ApiError;
use crate::state::AppState;

// ---- DTOs ----

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct CreatePageRequest {
    pub collection_id: Option<Uuid>,
    #[validate(length(min = 1, max = 500))]
    pub title: String,
    #[validate(length(min = 1))]
    pub content_markdown: String,
    /// BCP 47 language tag (e.g., "en-US", "pt-BR").
    #[validate(length(min = 2, max = 35))]
    pub language: String,
}

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct UpdatePageRequest {
    #[validate(length(min = 1, max = 500))]
    pub title: Option<String>,
    pub content_markdown: Option<String>,
    /// BCP 47 language tag. Defaults to "en" when omitted.
    #[validate(length(min = 2, max = 35))]
    pub language: Option<String>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct PageResponse {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub collection_id: Option<Uuid>,
    pub slug: String,
    pub status: DbPageStatus,
    pub created_by: Option<Uuid>,
    pub versions: Vec<PageVersionResponse>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct PageVersionResponse {
    pub id: Uuid,
    pub language: String,
    pub title: String,
    pub content_markdown: String,
    pub status: DbPageStatus,
    pub author_id: Option<Uuid>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct PublishResponse {
    pub page_id: Uuid,
    pub status: String,
    pub message: String,
}

impl From<PageVersion> for PageVersionResponse {
    fn from(v: PageVersion) -> Self {
        Self {
            id: v.id,
            language: v.language.into_string(),
            title: v.title,
            content_markdown: v.content_markdown,
            status: page_status_to_db(v.status),
            author_id: v.author_id,
            created_at: v.created_at.to_rfc3339(),
            updated_at: v.updated_at.to_rfc3339(),
        }
    }
}

fn page_status_to_db(s: PageStatus) -> DbPageStatus {
    match s {
        PageStatus::Draft => DbPageStatus::Draft,
        PageStatus::Published => DbPageStatus::Published,
    }
}

fn build_page_response(view: PageView) -> PageResponse {
    let PageView { page, versions } = view;
    let Page {
        id,
        workspace_id,
        collection_id,
        slug,
        status,
        created_by,
        created_at,
        updated_at,
    } = page;
    PageResponse {
        id,
        workspace_id,
        collection_id,
        slug: slug.into_string(),
        status: page_status_to_db(status),
        created_by,
        versions: versions.into_iter().map(Into::into).collect(),
        created_at: created_at.to_rfc3339(),
        updated_at: updated_at.to_rfc3339(),
    }
}

// ---- query params ----

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct ListPagesQuery {
    pub collection_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct SearchPagesQuery {
    pub q: String,
}

// ---- handlers ----

#[utoipa::path(
    get,
    path = "/pages",
    params(ListPagesQuery),
    responses(
        (status = 200, description = "pages list", body = Vec<PageResponse>),
        (status = 401, description = "unauthorized"),
    ),
    security(("bearer" = [])),
    tag = "pages"
)]
pub async fn list_pages(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Query(params): Query<ListPagesQuery>,
) -> Result<Json<Vec<PageResponse>>, ApiError> {
    let views = state
        .use_cases
        .list_pages
        .execute(auth.as_actor(), params.collection_id)
        .await?;
    Ok(Json(views.into_iter().map(build_page_response).collect()))
}

#[utoipa::path(
    get,
    path = "/pages/search",
    params(SearchPagesQuery),
    responses(
        (status = 200, description = "search results", body = Vec<PageResponse>),
        (status = 401, description = "unauthorized"),
    ),
    security(("bearer" = [])),
    tag = "pages"
)]
pub async fn search_pages(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Query(params): Query<SearchPagesQuery>,
) -> Result<Json<Vec<PageResponse>>, ApiError> {
    let views = state
        .use_cases
        .search_pages
        .execute(auth.as_actor(), &params.q)
        .await?;
    Ok(Json(views.into_iter().map(build_page_response).collect()))
}

#[utoipa::path(
    post,
    path = "/pages",
    request_body = CreatePageRequest,
    responses(
        (status = 201, description = "page created", body = PageResponse),
        (status = 400, description = "validation error"),
        (status = 401, description = "unauthorized"),
        (status = 403, description = "forbidden"),
        (status = 409, description = "slug conflict"),
    ),
    security(("bearer" = [])),
    tag = "pages"
)]
pub async fn create_page(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(body): Json<CreatePageRequest>,
) -> Result<(StatusCode, Json<PageResponse>), ApiError> {
    body.validate()
        .map_err(|e| ApiError::Validation(e.to_string()))?;

    let view = state
        .use_cases
        .create_page
        .execute(
            auth.as_actor(),
            CreatePageCommand {
                collection_id: body.collection_id,
                title: body.title,
                content_markdown: body.content_markdown,
                language: Language::from_trusted(body.language),
            },
        )
        .await?;

    Ok((StatusCode::CREATED, Json(build_page_response(view))))
}

#[utoipa::path(
    get,
    path = "/pages/{id}",
    params(("id" = Uuid, Path, description = "Page ID")),
    responses(
        (status = 200, description = "page with all versions", body = PageResponse),
        (status = 401, description = "unauthorized"),
        (status = 404, description = "not found"),
    ),
    security(("bearer" = [])),
    tag = "pages"
)]
pub async fn get_page(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<PageResponse>, ApiError> {
    let view = state.use_cases.get_page.execute(auth.as_actor(), id).await?;
    Ok(Json(build_page_response(view)))
}

#[utoipa::path(
    patch,
    path = "/pages/{id}",
    request_body = UpdatePageRequest,
    params(("id" = Uuid, Path, description = "Page ID")),
    responses(
        (status = 200, description = "page updated", body = PageResponse),
        (status = 400, description = "validation error or page is published"),
        (status = 401, description = "unauthorized"),
        (status = 403, description = "forbidden"),
        (status = 404, description = "not found"),
    ),
    security(("bearer" = [])),
    tag = "pages"
)]
pub async fn update_page(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdatePageRequest>,
) -> Result<Json<PageResponse>, ApiError> {
    body.validate()
        .map_err(|e| ApiError::Validation(e.to_string()))?;

    let view = state
        .use_cases
        .update_page
        .execute(
            auth.as_actor(),
            UpdatePageCommand {
                page_id: id,
                language: body.language.map(Language::from_trusted),
                title: body.title,
                content_markdown: body.content_markdown,
            },
        )
        .await?;
    Ok(Json(build_page_response(view)))
}

#[utoipa::path(
    post,
    path = "/pages/{id}/publish",
    params(("id" = Uuid, Path, description = "Page ID")),
    responses(
        (status = 202, description = "publish accepted, chunking in progress", body = PublishResponse),
        (status = 401, description = "unauthorized"),
        (status = 403, description = "forbidden"),
        (status = 404, description = "not found"),
    ),
    security(("bearer" = [])),
    tag = "pages"
)]
pub async fn publish_page(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(page_id): Path<Uuid>,
) -> Result<(StatusCode, Json<PublishResponse>), ApiError> {
    let actor = auth.as_actor();
    let confirmed_id = state
        .use_cases
        .publish_page
        .execute(actor, page_id)
        .await?;

    // Async chunking — fire-and-forget. Must stay in presentation,
    // not in the use case, because it owns a tokio runtime handle.
    let uc = state.use_cases.publish_page.clone();
    tokio::spawn(async move {
        uc.run_chunk_pipeline_for(confirmed_id, actor.workspace_id)
            .await;
    });

    Ok((
        StatusCode::ACCEPTED,
        Json(PublishResponse {
            page_id: confirmed_id,
            status: "published".into(),
            message: "page published; chunking in progress".into(),
        }),
    ))
}

#[utoipa::path(
    post,
    path = "/pages/{id}/draft",
    params(("id" = Uuid, Path, description = "Page ID")),
    responses(
        (status = 200, description = "reverted to draft", body = PageResponse),
        (status = 401, description = "unauthorized"),
        (status = 403, description = "forbidden"),
        (status = 404, description = "not found"),
    ),
    security(("bearer" = [])),
    tag = "pages"
)]
pub async fn draft_page(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(page_id): Path<Uuid>,
) -> Result<Json<PageResponse>, ApiError> {
    let view = state
        .use_cases
        .draft_page
        .execute(auth.as_actor(), page_id)
        .await?;
    // draft_page in the use case returns the current state, but its
    // status has not been flipped on the returned entity since the
    // use case reads it before the status change. The old handler
    // rewrote status to Draft on the DTO; do the same so wire-format
    // stability holds.
    let mut resp = build_page_response(view);
    resp.status = DbPageStatus::Draft;
    for v in &mut resp.versions {
        v.status = DbPageStatus::Draft;
    }
    Ok(Json(resp))
}

// ---- language completeness ----

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct PageVersionsResponse {
    pub page_id: Uuid,
    pub workspace_languages: Vec<String>,
    pub primary_language: String,
    pub versions: Vec<PageVersionResponse>,
    pub missing_languages: Vec<String>,
    pub complete: bool,
}

#[utoipa::path(
    get,
    path = "/pages/{id}/versions",
    params(("id" = Uuid, Path, description = "Page ID")),
    responses(
        (status = 200, description = "page versions with completeness metadata", body = PageVersionsResponse),
        (status = 401, description = "unauthorized"),
        (status = 404, description = "not found"),
    ),
    security(("bearer" = [])),
    tag = "pages"
)]
pub async fn get_page_versions(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<PageVersionsResponse>, ApiError> {
    let view = state
        .use_cases
        .get_page_versions
        .execute(auth.as_actor(), id)
        .await?;
    let complete = view.complete();
    let PageVersionsView {
        page,
        workspace_languages,
        primary_language,
        versions,
        missing_languages,
    } = view;
    Ok(Json(PageVersionsResponse {
        page_id: page.id,
        workspace_languages: workspace_languages
            .into_iter()
            .map(Language::into_string)
            .collect(),
        primary_language: primary_language.into_string(),
        versions: versions.into_iter().map(Into::into).collect(),
        missing_languages: missing_languages
            .into_iter()
            .map(Language::into_string)
            .collect(),
        complete,
    }))
}

// ---- version history ----

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct VersionHistoryQuery {
    /// BCP 47 language tag (required).
    pub language: String,
    /// Page number (1-indexed, default 1).
    pub page: Option<i64>,
    /// Items per page (default 20, max 50).
    pub per_page: Option<i64>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct VersionHistoryListResponse {
    pub page_id: Uuid,
    pub language: String,
    pub versions: Vec<VersionHistorySummary>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct VersionHistorySummary {
    pub id: Uuid,
    pub version_number: i32,
    pub title: String,
    pub content_preview: String,
    pub is_published: bool,
    pub author_id: Option<Uuid>,
    pub created_at: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct VersionHistoryDetailResponse {
    pub id: Uuid,
    pub page_id: Uuid,
    pub language: String,
    pub version_number: i32,
    pub title: String,
    pub content_markdown: String,
    pub is_published: bool,
    pub author_id: Option<Uuid>,
    pub created_at: String,
}

impl From<VersionHistoryEntry> for VersionHistoryDetailResponse {
    fn from(e: VersionHistoryEntry) -> Self {
        Self {
            id: e.id,
            page_id: e.page_id,
            language: e.language.into_string(),
            version_number: e.version_number,
            title: e.title,
            content_markdown: e.content_markdown,
            is_published: e.is_published,
            author_id: e.author_id,
            created_at: e.created_at.to_rfc3339(),
        }
    }
}

#[utoipa::path(
    get,
    path = "/pages/{id}/history",
    params(("id" = Uuid, Path, description = "Page ID"), VersionHistoryQuery),
    responses(
        (status = 200, description = "paginated version history", body = VersionHistoryListResponse),
        (status = 401, description = "unauthorized"),
        (status = 404, description = "not found"),
    ),
    security(("bearer" = [])),
    tag = "pages"
)]
pub async fn list_version_history(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Query(params): Query<VersionHistoryQuery>,
) -> Result<Json<VersionHistoryListResponse>, ApiError> {
    let language_str = params.language.clone();
    let page = state
        .use_cases
        .list_version_history
        .execute(
            auth.as_actor(),
            ListVersionHistoryCommand {
                page_id: id,
                language: Language::from_trusted(params.language),
                page: params.page,
                per_page: params.per_page,
            },
        )
        .await?;

    let VersionHistoryPage {
        summaries,
        total,
        page: pg,
        per_page,
    } = page;

    let versions = summaries
        .into_iter()
        .map(|s| VersionHistorySummary {
            id: s.id,
            version_number: s.version_number,
            title: s.title,
            content_preview: s.content_preview,
            is_published: s.is_published,
            author_id: s.author_id,
            created_at: s.created_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(VersionHistoryListResponse {
        page_id: id,
        language: language_str,
        versions,
        total,
        page: pg,
        per_page,
    }))
}

#[utoipa::path(
    get,
    path = "/pages/{id}/history/{history_id}",
    params(
        ("id" = Uuid, Path, description = "Page ID"),
        ("history_id" = Uuid, Path, description = "Version history entry ID"),
    ),
    responses(
        (status = 200, description = "full version content", body = VersionHistoryDetailResponse),
        (status = 401, description = "unauthorized"),
        (status = 404, description = "not found"),
    ),
    security(("bearer" = [])),
    tag = "pages"
)]
pub async fn get_version_history_item(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((id, history_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<VersionHistoryDetailResponse>, ApiError> {
    let entry = state
        .use_cases
        .get_version_history_item
        .execute(auth.as_actor(), id, history_id)
        .await?;
    Ok(Json(entry.into()))
}

#[utoipa::path(
    post,
    path = "/pages/{id}/history/{history_id}/restore",
    params(
        ("id" = Uuid, Path, description = "Page ID"),
        ("history_id" = Uuid, Path, description = "Version history entry ID"),
    ),
    responses(
        (status = 200, description = "restored as draft", body = PageResponse),
        (status = 400, description = "page is published"),
        (status = 401, description = "unauthorized"),
        (status = 403, description = "forbidden"),
        (status = 404, description = "not found"),
    ),
    security(("bearer" = [])),
    tag = "pages"
)]
pub async fn restore_version(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path((id, history_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<PageResponse>, ApiError> {
    let view = state
        .use_cases
        .restore_version
        .execute(auth.as_actor(), id, history_id)
        .await?;
    Ok(Json(build_page_response(view)))
}
