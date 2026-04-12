//! `POST /admin/users/invite` — create a pending user row and
//! return an activation link for the admin to share out of band.
//!
//! v1 does not send email; the admin copies the `activation_url`
//! from the response and delivers it to the invitee through
//! whatever channel they already use (Slack, email, carrier pigeon).

use std::sync::Arc;

use axum::{extract::State, Json};
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
