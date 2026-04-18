//! `/collections` HTTP handlers — thin Clean Architecture adapters
//! that translate DTOs, call use cases, and map the domain result
//! back onto the wire. All business logic lives in
//! [`crate::application::collections`].

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::application::collections::{CreateCollectionCommand, UpdateCollectionCommand};
use crate::presentation::extractor::AuthUser;
use crate::domain::entity::Collection;
use crate::presentation::error::ApiError;
use crate::state::AppState;

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

impl From<Collection> for CollectionResponse {
    fn from(c: Collection) -> Self {
        Self {
            id: c.id,
            workspace_id: c.workspace_id,
            parent_id: c.parent_id,
            name: c.name,
            slug: c.slug.into_string(),
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
    body.validate()
        .map_err(|e| ApiError::Validation(e.to_string()))?;

    let collection = state
        .use_cases
        .create_collection
        .execute(
            auth.as_actor(),
            CreateCollectionCommand {
                name: body.name,
                parent_id: body.parent_id,
            },
        )
        .await?;

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
    let rows = state
        .use_cases
        .list_collections
        .execute(auth.as_actor())
        .await?;
    Ok(Json(rows.into_iter().map(Into::into).collect()))
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
    body.validate()
        .map_err(|e| ApiError::Validation(e.to_string()))?;

    let collection = state
        .use_cases
        .update_collection
        .execute(
            auth.as_actor(),
            UpdateCollectionCommand {
                id,
                name: body.name,
                parent_id: body.parent_id,
            },
        )
        .await?;

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
    state
        .use_cases
        .delete_collection
        .execute(auth.as_actor(), id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
