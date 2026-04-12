//! `POST /admin/users/invite` — create a pending user row and
//! return an activation link for the admin to share out of band.
//!
//! v1 does not send email; the admin copies the `activation_url`
//! from the response and delivers it to the invitee through
//! whatever channel they already use (Slack, email, carrier pigeon).

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::{DateTime, Duration, Utc};
use historiador_db::postgres::users::{self, Role};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::auth::{
    extractor::AuthUser,
    rbac::require_role,
    tokens::{self, INVITE_TOKEN_TTL_DAYS},
};
use crate::error::ApiError;
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
    require_role(&auth, Role::Admin)?;
    body.validate()
        .map_err(|e| ApiError::Validation(e.to_string()))?;

    // Refuse if an account already exists for this email in the
    // caller's workspace — whether activated or pending.
    if users::find_by_email(&state.pool, auth.workspace_id, &body.email)
        .await
        .map_err(ApiError::Internal)?
        .is_some()
    {
        return Err(ApiError::Conflict(
            "a user with this email already exists in the workspace".into(),
        ));
    }

    let (invite_token_plaintext, invite_token_hash) = tokens::generate();
    let expires_at = Utc::now() + Duration::days(INVITE_TOKEN_TTL_DAYS);

    let user_id = users::insert_pending(
        &state.pool,
        auth.workspace_id,
        &body.email,
        body.role,
        &invite_token_hash,
        expires_at,
    )
    .await
    .map_err(ApiError::Internal)?;

    let activation_url = format!(
        "{}/activate?token={}",
        state.public_base_url.trim_end_matches('/'),
        invite_token_plaintext
    );

    Ok(Json(InviteResponse {
        user_id,
        activation_url,
        expires_at,
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
    require_role(&auth, Role::Admin)?;

    let rows = users::list_by_workspace(&state.pool, auth.workspace_id)
        .await
        .map_err(ApiError::Internal)?;

    let results: Vec<UserResponse> = rows
        .into_iter()
        .map(|u| UserResponse {
            id: u.id,
            email: u.email,
            role: u.role,
            active: u.active,
            pending: u.password_hash.is_none(),
        })
        .collect();

    Ok(Json(results))
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
    require_role(&auth, Role::Admin)?;

    let affected = users::deactivate(&state.pool, user_id, auth.workspace_id)
        .await
        .map_err(ApiError::Internal)?;

    if affected == 0 {
        return Err(ApiError::NotFound);
    }

    Ok(StatusCode::NO_CONTENT)
}
