//! `/admin/users*` HTTP handlers — thin Clean Architecture wrappers
//! over [`crate::application::admin`].

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::{DateTime, Utc};
use historiador_db::postgres::users::Role;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::application::admin::InviteUserCommand;
use crate::domain::value::{Email, Role as DomainRole};
use crate::presentation::error::ApiError;
use crate::presentation::extractor::AuthUser;
use crate::state::AppState;

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct InviteRequest {
    #[validate(email)]
    pub email: String,
    pub role: Role,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct InviteResponse {
    pub user_id: Uuid,
    pub activation_url: String,
    pub expires_at: DateTime<Utc>,
}

#[utoipa::path(
    post,
    path = "/admin/users/invite",
    request_body = InviteRequest,
    responses(
        (status = 200, description = "invited", body = InviteResponse),
        (status = 400, description = "validation error"),
        (status = 401, description = "unauthenticated"),
        (status = 403, description = "caller is not admin"),
        (status = 409, description = "email already in use"),
    ),
    tag = "admin",
    security(("bearer" = []))
)]
pub async fn invite(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(body): Json<InviteRequest>,
) -> Result<Json<InviteResponse>, ApiError> {
    body.validate()
        .map_err(|e| ApiError::Validation(e.to_string()))?;

    let result = state
        .use_cases
        .invite_user
        .execute(
            auth.as_actor(),
            InviteUserCommand {
                email: Email::parse(body.email)
                    .map_err(|e| ApiError::Validation(format!("{e}")))?,
                role: db_role_to_domain(body.role),
            },
        )
        .await?;

    let activation_url = format!(
        "{}/activate?token={}",
        state.public_base_url.trim_end_matches('/'),
        result.invite_token
    );

    Ok(Json(InviteResponse {
        user_id: result.user_id,
        activation_url,
        expires_at: result.expires_at,
    }))
}

// ---- list / deactivate ----

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct UserResponse {
    pub id: Uuid,
    pub email: String,
    pub role: Role,
    pub active: bool,
    /// `true` if the user has not yet activated their account.
    pub pending: bool,
}

#[utoipa::path(
    get,
    path = "/admin/users",
    responses(
        (status = 200, description = "user list", body = Vec<UserResponse>),
        (status = 401, description = "unauthenticated"),
        (status = 403, description = "caller is not admin"),
    ),
    tag = "admin",
    security(("bearer" = []))
)]
pub async fn list_users(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> Result<Json<Vec<UserResponse>>, ApiError> {
    let rows = state.use_cases.list_users.execute(auth.as_actor()).await?;
    let out = rows
        .into_iter()
        .map(|u| UserResponse {
            id: u.id,
            email: u.email.into_string(),
            role: domain_role_to_db(u.role),
            active: u.active,
            pending: u.password_hash.is_none(),
        })
        .collect();
    Ok(Json(out))
}

#[utoipa::path(
    patch,
    path = "/admin/users/{id}/deactivate",
    params(("id" = Uuid, Path, description = "User ID")),
    responses(
        (status = 204, description = "user deactivated"),
        (status = 401, description = "unauthenticated"),
        (status = 403, description = "caller is not admin"),
        (status = 404, description = "user not found"),
    ),
    tag = "admin",
    security(("bearer" = []))
)]
pub async fn deactivate_user(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(user_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    state
        .use_cases
        .deactivate_user
        .execute(auth.as_actor(), user_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

fn db_role_to_domain(r: Role) -> DomainRole {
    match r {
        Role::Admin => DomainRole::Admin,
        Role::Author => DomainRole::Author,
        Role::Viewer => DomainRole::Viewer,
    }
}

fn domain_role_to_db(r: DomainRole) -> Role {
    match r {
        DomainRole::Admin => Role::Admin,
        DomainRole::Author => Role::Author,
        DomainRole::Viewer => Role::Viewer,
    }
}
