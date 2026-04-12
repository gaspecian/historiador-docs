//! `/collections` HTTP handlers.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use historiador_db::postgres::{collections, users::Role};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::auth::{extractor::AuthUser, rbac::require_role};
use crate::error::ApiError;
use crate::state::AppState;
use crate::util::slugify;

// ---- DTOs ----

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct CreateCollectionRequest {
    #[validate(length(min = 1, max = 200))]
    pub name: String,
    pub parent_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct UpdateCollectionRequest {
    #[validate(length(min = 1, max = 200))]
    pub name: Option<String>,
    /// Set to `null` to move to root, or a UUID to move under a parent.
    /// Omit the field entirely to leave unchanged.
    pub parent_id: Option<Option<Uuid>>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct CollectionResponse {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub name: String,
    pub slug: String,
    pub sort_order: i32,
    pub created_at: String,
    pub updated_at: String,
}

impl From<collections::Collection> for CollectionResponse {
    fn from(c: collections::Collection) -> Self {
        Self {
            id: c.id,
            workspace_id: c.workspace_id,
            parent_id: c.parent_id,
            name: c.name,
            slug: c.slug,
            sort_order: c.sort_order,
            created_at: c.created_at.to_rfc3339(),
            updated_at: c.updated_at.to_rfc3339(),
        }
    }
}

// ---- handlers ----

#[utoipa::path(
    post,
    path = "/collections",
    request_body = CreateCollectionRequest,
    responses(
        (status = 201, description = "collection created", body = CollectionResponse),
        (status = 400, description = "validation error"),
        (status = 401, description = "unauthorized"),
        (status = 403, description = "forbidden"),
        (status = 409, description = "slug conflict"),
    ),
    security(("bearer" = [])),
    tag = "collections"
)]
pub async fn create_collection(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(body): Json<CreateCollectionRequest>,
) -> Result<(StatusCode, Json<CollectionResponse>), ApiError> {
    require_role(&auth, Role::Author)?;
    body.validate()
        .map_err(|e| ApiError::Validation(e.to_string()))?;

    // Validate parent exists if specified.
    if let Some(parent_id) = body.parent_id {
        collections::find_by_id(&state.pool, parent_id, auth.workspace_id)
            .await
            .map_err(ApiError::Internal)?
            .ok_or_else(|| ApiError::Validation("parent collection not found".into()))?;
    }

    let slug = slugify(&body.name);

    let collection = collections::insert(
        &state.pool,
        auth.workspace_id,
        body.parent_id,
        &body.name,
        &slug,
    )
    .await
    .map_err(|e| {
        // Check for unique constraint violation (slug conflict).
        let msg = e.to_string();
        if msg.contains("duplicate key") || msg.contains("unique constraint") {
            ApiError::Conflict(format!("collection with slug '{slug}' already exists"))
        } else {
            ApiError::Internal(e)
        }
    })?;

    Ok((StatusCode::CREATED, Json(collection.into())))
}

#[utoipa::path(
    get,
    path = "/collections",
    responses(
        (status = 200, description = "list of collections", body = Vec<CollectionResponse>),
        (status = 401, description = "unauthorized"),
    ),
    security(("bearer" = [])),
    tag = "collections"
)]
pub async fn list_collections(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> Result<Json<Vec<CollectionResponse>>, ApiError> {
    require_role(&auth, Role::Viewer)?;

    let rows = collections::list_by_workspace(&state.pool, auth.workspace_id)
        .await
        .map_err(ApiError::Internal)?;

    let resp: Vec<CollectionResponse> = rows.into_iter().map(Into::into).collect();
    Ok(Json(resp))
}

#[utoipa::path(
    patch,
    path = "/collections/{id}",
    request_body = UpdateCollectionRequest,
    params(("id" = Uuid, Path, description = "Collection ID")),
    responses(
        (status = 200, description = "collection updated", body = CollectionResponse),
        (status = 400, description = "validation error"),
        (status = 401, description = "unauthorized"),
        (status = 403, description = "forbidden"),
        (status = 404, description = "not found"),
        (status = 409, description = "slug conflict"),
    ),
    security(("bearer" = [])),
    tag = "collections"
)]
pub async fn update_collection(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateCollectionRequest>,
) -> Result<Json<CollectionResponse>, ApiError> {
    require_role(&auth, Role::Author)?;
    body.validate()
        .map_err(|e| ApiError::Validation(e.to_string()))?;

    let new_slug = body.name.as_deref().map(slugify);

    let collection = collections::update(
        &state.pool,
        id,
        auth.workspace_id,
        body.name.as_deref(),
        new_slug.as_deref(),
        body.parent_id,
    )
    .await
    .map_err(|e| {
        let msg = e.to_string();
        if msg.contains("duplicate key") || msg.contains("unique constraint") {
            ApiError::Conflict("collection slug conflict".into())
        } else {
            ApiError::Internal(e)
        }
    })?
    .ok_or(ApiError::NotFound)?;

    Ok(Json(collection.into()))
}

#[utoipa::path(
    delete,
    path = "/collections/{id}",
    params(("id" = Uuid, Path, description = "Collection ID")),
    responses(
        (status = 204, description = "collection deleted"),
        (status = 401, description = "unauthorized"),
        (status = 403, description = "forbidden"),
        (status = 404, description = "not found"),
    ),
    security(("bearer" = [])),
    tag = "collections"
)]
pub async fn delete_collection(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    require_role(&auth, Role::Author)?;

    let deleted =
        collections::delete_cascade(&state.pool, id, auth.workspace_id)
            .await
            .map_err(ApiError::Internal)?;

    if deleted == 0 {
        return Err(ApiError::NotFound);
    }

    Ok(StatusCode::NO_CONTENT)
}
