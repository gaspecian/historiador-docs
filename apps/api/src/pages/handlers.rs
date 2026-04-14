//! `/pages` HTTP handlers.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use historiador_db::postgres::{
    page_version_history,
    page_versions::{self, PageVersion},
    pages::{self, PageStatus},
    users::Role,
    workspaces,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::auth::{extractor::AuthUser, rbac::require_role};
use crate::error::ApiError;
use crate::state::AppState;
use crate::util::slugify;

use super::pipeline;

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
    /// BCP 47 language tag. Defaults to the version's current language.
    #[validate(length(min = 2, max = 35))]
    pub language: Option<String>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct PageResponse {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub collection_id: Option<Uuid>,
    pub slug: String,
    pub status: PageStatus,
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
    pub status: PageStatus,
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
            language: v.language,
            title: v.title,
            content_markdown: v.content_markdown,
            status: v.status,
            author_id: v.author_id,
            created_at: v.created_at.to_rfc3339(),
            updated_at: v.updated_at.to_rfc3339(),
        }
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
    axum::extract::Query(params): axum::extract::Query<ListPagesQuery>,
) -> Result<Json<Vec<PageResponse>>, ApiError> {
    require_role(&auth, Role::Viewer)?;

    let pages_list =
        pages::list_by_collection(&state.pool, auth.workspace_id, params.collection_id)
            .await
            .map_err(ApiError::Internal)?;

    let mut results = Vec::with_capacity(pages_list.len());
    for page in pages_list {
        let versions = page_versions::find_by_page(&state.pool, page.id)
            .await
            .map_err(ApiError::Internal)?;
        results.push(PageResponse {
            id: page.id,
            workspace_id: page.workspace_id,
            collection_id: page.collection_id,
            slug: page.slug,
            status: page.status,
            created_by: page.created_by,
            versions: versions.into_iter().map(Into::into).collect(),
            created_at: page.created_at.to_rfc3339(),
            updated_at: page.updated_at.to_rfc3339(),
        });
    }

    Ok(Json(results))
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
    axum::extract::Query(params): axum::extract::Query<SearchPagesQuery>,
) -> Result<Json<Vec<PageResponse>>, ApiError> {
    require_role(&auth, Role::Viewer)?;

    let pages_list = pages::search(&state.pool, auth.workspace_id, &params.q)
        .await
        .map_err(ApiError::Internal)?;

    let mut results = Vec::with_capacity(pages_list.len());
    for page in pages_list {
        let versions = page_versions::find_by_page(&state.pool, page.id)
            .await
            .map_err(ApiError::Internal)?;
        results.push(PageResponse {
            id: page.id,
            workspace_id: page.workspace_id,
            collection_id: page.collection_id,
            slug: page.slug,
            status: page.status,
            created_by: page.created_by,
            versions: versions.into_iter().map(Into::into).collect(),
            created_at: page.created_at.to_rfc3339(),
            updated_at: page.updated_at.to_rfc3339(),
        });
    }

    Ok(Json(results))
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
    require_role(&auth, Role::Author)?;
    body.validate()
        .map_err(|e| ApiError::Validation(e.to_string()))?;

    let slug = slugify(&body.title);

    let page = pages::insert(
        &state.pool,
        auth.workspace_id,
        body.collection_id,
        &slug,
        auth.user_id,
    )
    .await
    .map_err(|e| {
        let msg = e.to_string();
        if msg.contains("duplicate key") || msg.contains("unique constraint") {
            ApiError::Conflict(format!(
                "page with slug '{slug}' already exists in this collection"
            ))
        } else {
            ApiError::Internal(e)
        }
    })?;

    // Create the initial draft version for the given language.
    let version = page_versions::upsert(
        &state.pool,
        page.id,
        &body.language,
        &body.title,
        &body.content_markdown,
        auth.user_id,
        PageStatus::Draft,
    )
    .await
    .map_err(ApiError::Internal)?;

    // Fire-and-forget: emit page.created event to Chronik.
    if let Some(ref chronik) = state.chronik {
        chronik.produce_event_fire_and_forget(
            historiador_db::chronik::producer::topics::PAGE_EVENTS,
            page.id.to_string(),
            serde_json::json!({
                "event": "page.created",
                "page_id": page.id,
                "language": body.language,
                "title": body.title,
                "author_id": auth.user_id,
                "timestamp": chrono::Utc::now().to_rfc3339(),
            }),
        );
    }

    let resp = PageResponse {
        id: page.id,
        workspace_id: page.workspace_id,
        collection_id: page.collection_id,
        slug: page.slug,
        status: page.status,
        created_by: page.created_by,
        versions: vec![version.into()],
        created_at: page.created_at.to_rfc3339(),
        updated_at: page.updated_at.to_rfc3339(),
    };

    Ok((StatusCode::CREATED, Json(resp)))
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
    require_role(&auth, Role::Viewer)?;

    let page = pages::find_by_id(&state.pool, id, auth.workspace_id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;

    let versions = page_versions::find_by_page(&state.pool, page.id)
        .await
        .map_err(ApiError::Internal)?;

    let resp = PageResponse {
        id: page.id,
        workspace_id: page.workspace_id,
        collection_id: page.collection_id,
        slug: page.slug,
        status: page.status,
        created_by: page.created_by,
        versions: versions.into_iter().map(Into::into).collect(),
        created_at: page.created_at.to_rfc3339(),
        updated_at: page.updated_at.to_rfc3339(),
    };

    Ok(Json(resp))
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
    require_role(&auth, Role::Author)?;
    body.validate()
        .map_err(|e| ApiError::Validation(e.to_string()))?;

    let page = pages::find_by_id(&state.pool, id, auth.workspace_id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;

    if page.status == PageStatus::Published {
        return Err(ApiError::Validation(
            "page is published — revert to draft before editing".into(),
        ));
    }

    // Determine which language version to update.
    // Default to "en" if no language specified. In practice, callers
    // should pass the language explicitly for the version they want.
    let language = body.language.as_deref().unwrap_or("en");

    // Load the existing version (if any) to merge partial updates.
    let existing = page_versions::find_by_page_and_language(&state.pool, page.id, language)
        .await
        .map_err(ApiError::Internal)?;

    let title = body
        .title
        .as_deref()
        .or(existing.as_ref().map(|v| v.title.as_str()))
        .unwrap_or("Untitled");
    let content = body
        .content_markdown
        .as_deref()
        .or(existing.as_ref().map(|v| v.content_markdown.as_str()))
        .unwrap_or("");

    page_versions::upsert(
        &state.pool,
        page.id,
        language,
        title,
        content,
        auth.user_id,
        PageStatus::Draft,
    )
    .await
    .map_err(ApiError::Internal)?;

    // Snapshot to version history (debounced: skip if < 30s since last).
    let should_snapshot =
        !page_version_history::has_recent_snapshot(&state.pool, page.id, language, 30)
            .await
            .unwrap_or(true);

    if should_snapshot {
        if let Err(e) = page_version_history::insert(
            &state.pool,
            page.id,
            language,
            title,
            content,
            false, // is_published
            Some(auth.user_id),
        )
        .await
        {
            tracing::warn!(page_id = %page.id, error = ?e, "failed to snapshot version history on save");
        }
    }

    // Fire-and-forget: emit page.updated event to Chronik.
    if let Some(ref chronik) = state.chronik {
        chronik.produce_event_fire_and_forget(
            historiador_db::chronik::producer::topics::PAGE_EVENTS,
            page.id.to_string(),
            serde_json::json!({
                "event": "page.updated",
                "page_id": page.id,
                "language": language,
                "title": title,
                "author_id": auth.user_id,
                "timestamp": chrono::Utc::now().to_rfc3339(),
            }),
        );
    }

    // Re-fetch all versions for the response.
    let versions = page_versions::find_by_page(&state.pool, page.id)
        .await
        .map_err(ApiError::Internal)?;

    let resp = PageResponse {
        id: page.id,
        workspace_id: page.workspace_id,
        collection_id: page.collection_id,
        slug: page.slug,
        status: page.status,
        created_by: page.created_by,
        versions: versions.into_iter().map(Into::into).collect(),
        created_at: page.created_at.to_rfc3339(),
        updated_at: page.updated_at.to_rfc3339(),
    };

    Ok(Json(resp))
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
    require_role(&auth, Role::Author)?;

    let page = pages::find_by_id(&state.pool, page_id, auth.workspace_id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;

    // Synchronously update status.
    pages::update_status(
        &state.pool,
        page.id,
        auth.workspace_id,
        PageStatus::Published,
    )
    .await
    .map_err(ApiError::Internal)?;
    page_versions::update_status_all(&state.pool, page.id, PageStatus::Published)
        .await
        .map_err(ApiError::Internal)?;

    // Fetch all versions for async processing.
    let versions = page_versions::find_by_page(&state.pool, page.id)
        .await
        .map_err(ApiError::Internal)?;

    // Snapshot each language version into the history table.
    for v in &versions {
        if let Err(e) = page_version_history::insert(
            &state.pool,
            v.page_id,
            &v.language,
            &v.title,
            &v.content_markdown,
            true, // is_published
            v.author_id,
        )
        .await
        {
            tracing::warn!(page_id = %v.page_id, language = %v.language, error = ?e, "failed to snapshot version history on publish");
        }
    }

    // Fire-and-forget: emit page.published events to Chronik.
    if let Some(ref chronik) = state.chronik {
        for v in &versions {
            chronik.produce_event_fire_and_forget(
                historiador_db::chronik::producer::topics::PAGE_EVENTS,
                page.id.to_string(),
                serde_json::json!({
                    "event": "page.published",
                    "page_id": page.id,
                    "language": v.language,
                    "title": v.title,
                    "author_id": auth.user_id,
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                }),
            );
        }
    }

    // Fire-and-forget: spawn async chunking pipeline.
    let pool = state.pool.clone();
    let vector_store = state.vector_store.clone();
    let embedding_client = state.embedding_client.clone();
    tokio::spawn(async move {
        for version in versions {
            if let Err(e) = pipeline::run_chunk_pipeline(
                &pool,
                vector_store.as_ref(),
                embedding_client.as_ref(),
                &version,
            )
            .await
            {
                tracing::error!(
                    page_version_id = %version.id,
                    error = ?e,
                    "async chunk pipeline failed"
                );
            }
        }
    });

    Ok((
        StatusCode::ACCEPTED,
        Json(PublishResponse {
            page_id: page.id,
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
    require_role(&auth, Role::Author)?;

    let page = pages::find_by_id(&state.pool, page_id, auth.workspace_id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;

    // Revert status to draft.
    pages::update_status(&state.pool, page.id, auth.workspace_id, PageStatus::Draft)
        .await
        .map_err(ApiError::Internal)?;
    page_versions::update_status_all(&state.pool, page.id, PageStatus::Draft)
        .await
        .map_err(ApiError::Internal)?;

    // Delete existing chunks for all versions.
    let versions = page_versions::find_by_page(&state.pool, page.id)
        .await
        .map_err(ApiError::Internal)?;

    for version in &versions {
        // Delete from vector store.
        if let Err(e) = state
            .vector_store
            .delete_by_page_version(&version.id.to_string())
            .await
        {
            tracing::warn!(
                page_version_id = %version.id,
                error = ?e,
                "failed to delete chunks from vector store"
            );
        }
        // Delete from Postgres.
        historiador_db::postgres::chunks::delete_by_page_version(&state.pool, version.id)
            .await
            .map_err(ApiError::Internal)?;
    }

    // Fire-and-forget: emit page.unpublished event to Chronik.
    if let Some(ref chronik) = state.chronik {
        chronik.produce_event_fire_and_forget(
            historiador_db::chronik::producer::topics::PAGE_EVENTS,
            page.id.to_string(),
            serde_json::json!({
                "event": "page.unpublished",
                "page_id": page.id,
                "author_id": auth.user_id,
                "timestamp": chrono::Utc::now().to_rfc3339(),
            }),
        );
    }

    let resp = PageResponse {
        id: page.id,
        workspace_id: page.workspace_id,
        collection_id: page.collection_id,
        slug: page.slug,
        status: PageStatus::Draft,
        created_by: page.created_by,
        versions: versions.into_iter().map(Into::into).collect(),
        created_at: page.created_at.to_rfc3339(),
        updated_at: page.updated_at.to_rfc3339(),
    };

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
    require_role(&auth, Role::Viewer)?;

    let page = pages::find_by_id(&state.pool, id, auth.workspace_id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;

    let ws = workspaces::find_by_id(&state.pool, auth.workspace_id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;

    let versions = page_versions::find_by_page(&state.pool, page.id)
        .await
        .map_err(ApiError::Internal)?;

    let existing_langs: std::collections::HashSet<&str> =
        versions.iter().map(|v| v.language.as_str()).collect();
    let missing_languages: Vec<String> = ws
        .languages
        .iter()
        .filter(|lang| !existing_langs.contains(lang.as_str()))
        .cloned()
        .collect();
    let complete = missing_languages.is_empty();

    Ok(Json(PageVersionsResponse {
        page_id: page.id,
        workspace_languages: ws.languages,
        primary_language: ws.primary_language,
        versions: versions.into_iter().map(Into::into).collect(),
        missing_languages,
        complete,
    }))
}

// ---- version history (Sprint 7) ----

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

#[utoipa::path(
    get,
    path = "/pages/{id}/history",
    params(
        ("id" = Uuid, Path, description = "Page ID"),
        VersionHistoryQuery,
    ),
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
    axum::extract::Query(params): axum::extract::Query<VersionHistoryQuery>,
) -> Result<Json<VersionHistoryListResponse>, ApiError> {
    require_role(&auth, Role::Viewer)?;

    let page = pages::find_by_id(&state.pool, id, auth.workspace_id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;

    let pg = params.page.unwrap_or(1).max(1);
    let per_pg = params.per_page.unwrap_or(20).clamp(1, 50);

    let (summaries, total) = page_version_history::list_by_page_and_language(
        &state.pool,
        page.id,
        &params.language,
        pg,
        per_pg,
    )
    .await
    .map_err(ApiError::Internal)?;

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
        page_id: page.id,
        language: params.language,
        versions,
        total,
        page: pg,
        per_page: per_pg,
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
    require_role(&auth, Role::Viewer)?;

    // Verify page belongs to workspace.
    let _page = pages::find_by_id(&state.pool, id, auth.workspace_id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;

    let entry = page_version_history::find_by_id(&state.pool, history_id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;

    // Verify the history entry belongs to this page.
    if entry.page_id != id {
        return Err(ApiError::NotFound);
    }

    Ok(Json(VersionHistoryDetailResponse {
        id: entry.id,
        page_id: entry.page_id,
        language: entry.language,
        version_number: entry.version_number,
        title: entry.title,
        content_markdown: entry.content_markdown,
        is_published: entry.is_published,
        author_id: entry.author_id,
        created_at: entry.created_at.to_rfc3339(),
    }))
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
    require_role(&auth, Role::Author)?;

    let page = pages::find_by_id(&state.pool, id, auth.workspace_id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;

    if page.status == PageStatus::Published {
        return Err(ApiError::Validation(
            "page is published — revert to draft before restoring".into(),
        ));
    }

    let entry = page_version_history::find_by_id(&state.pool, history_id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;

    if entry.page_id != id {
        return Err(ApiError::NotFound);
    }

    // Upsert the historical content as the current draft.
    page_versions::upsert(
        &state.pool,
        page.id,
        &entry.language,
        &entry.title,
        &entry.content_markdown,
        auth.user_id,
        PageStatus::Draft,
    )
    .await
    .map_err(ApiError::Internal)?;

    // Record the restore itself in version history.
    let _ = page_version_history::insert(
        &state.pool,
        page.id,
        &entry.language,
        &entry.title,
        &entry.content_markdown,
        false,
        Some(auth.user_id),
    )
    .await;

    // Fire-and-forget: emit page.restored event to Chronik.
    if let Some(ref chronik) = state.chronik {
        chronik.produce_event_fire_and_forget(
            historiador_db::chronik::producer::topics::PAGE_EVENTS,
            page.id.to_string(),
            serde_json::json!({
                "event": "page.restored",
                "page_id": page.id,
                "language": entry.language,
                "restored_version_number": entry.version_number,
                "author_id": auth.user_id,
                "timestamp": chrono::Utc::now().to_rfc3339(),
            }),
        );
    }

    // Re-fetch for response.
    let versions = page_versions::find_by_page(&state.pool, page.id)
        .await
        .map_err(ApiError::Internal)?;

    Ok(Json(PageResponse {
        id: page.id,
        workspace_id: page.workspace_id,
        collection_id: page.collection_id,
        slug: page.slug,
        status: page.status,
        created_by: page.created_by,
        versions: versions.into_iter().map(Into::into).collect(),
        created_at: page.created_at.to_rfc3339(),
        updated_at: page.updated_at.to_rfc3339(),
    }))
}
