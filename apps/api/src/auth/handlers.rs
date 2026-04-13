//! `/auth/{login,refresh,logout,activate}` HTTP handlers.

use std::sync::Arc;

use axum::{extract::State, http::StatusCode, Json};
use chrono::{Duration, Utc};
use historiador_db::{
    password,
    postgres::{sessions, users},
};
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::auth::{
    jwt::{self, ACCESS_TOKEN_TTL_SECONDS},
    tokens,
};
use crate::error::ApiError;
use crate::state::AppState;

// ---- request / response DTOs ----

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct LoginRequest {
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 1))]
    pub password: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i64,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct LogoutRequest {
    pub refresh_token: String,
}

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct ActivateRequest {
    #[validate(length(min = 1))]
    pub invite_token: String,
    #[validate(length(min = 12))]
    pub password: String,
}

// ---- shared helpers ----

async fn issue_token_pair(
    state: &AppState,
    user_id: uuid::Uuid,
    workspace_id: uuid::Uuid,
    role: historiador_db::postgres::users::Role,
) -> Result<TokenResponse, ApiError> {
    let claims = jwt::Claims::new(user_id, workspace_id, role);
    let access_token = jwt::encode_token(&claims, &state.jwt_secret)
        .map_err(|e| ApiError::Internal(e.context("failed to encode access token")))?;

    let (refresh_plaintext, refresh_hash) = tokens::generate();
    let refresh_expires_at = Utc::now() + Duration::days(tokens::REFRESH_TOKEN_TTL_DAYS);
    sessions::insert(&state.pool, user_id, &refresh_hash, refresh_expires_at)
        .await
        .map_err(ApiError::Internal)?;

    Ok(TokenResponse {
        access_token,
        refresh_token: refresh_plaintext,
        expires_in: ACCESS_TOKEN_TTL_SECONDS,
    })
}

// ---- handlers ----

#[utoipa::path(
    post,
    path = "/auth/login",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "authenticated", body = TokenResponse),
        (status = 400, description = "validation error"),
        (status = 401, description = "invalid credentials"),
    ),
    tag = "auth"
)]
pub async fn login(
    State(state): State<Arc<AppState>>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<TokenResponse>, ApiError> {
    body.validate()
        .map_err(|e| ApiError::Validation(e.to_string()))?;

    // v1 is single-workspace. Pick the (only) workspace by looking up
    // the user across all workspaces that share the email. Since the
    // UNIQUE constraint is per-workspace, in a single-workspace install
    // there is at most one match.
    let user = sqlx::query_as::<_, users::User>(
        "SELECT id, workspace_id, email, password_hash, role, active, \
                invite_token_hash, invite_expires_at \
           FROM users \
          WHERE email = $1",
    )
    .bind(&body.email)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.into()))?
    .ok_or(ApiError::Unauthorized)?;

    if !user.active {
        return Err(ApiError::Unauthorized);
    }
    let stored_hash = user
        .password_hash
        .as_deref()
        .ok_or(ApiError::Unauthorized)?;
    let matches =
        password::verify(&body.password, stored_hash).map_err(|_| ApiError::Unauthorized)?;
    if !matches {
        return Err(ApiError::Unauthorized);
    }

    let tokens = issue_token_pair(&state, user.id, user.workspace_id, user.role).await?;
    Ok(Json(tokens))
}

#[utoipa::path(
    post,
    path = "/auth/refresh",
    request_body = RefreshRequest,
    responses(
        (status = 200, description = "rotated", body = TokenResponse),
        (status = 401, description = "invalid or expired refresh token"),
    ),
    tag = "auth"
)]
pub async fn refresh(
    State(state): State<Arc<AppState>>,
    Json(body): Json<RefreshRequest>,
) -> Result<Json<TokenResponse>, ApiError> {
    let hash = tokens::sha256_hex(&body.refresh_token);
    let session = sessions::find_active_by_token_hash(&state.pool, &hash)
        .await
        .map_err(ApiError::Internal)?
        .ok_or(ApiError::Unauthorized)?;

    // Rotate: delete the old session row, then issue a fresh pair.
    sessions::delete_by_token_hash(&state.pool, &hash)
        .await
        .map_err(ApiError::Internal)?;

    // Re-load the user so we have a fresh role snapshot (an admin may
    // have demoted them since login).
    let user = sqlx::query_as::<_, users::User>(
        "SELECT id, workspace_id, email, password_hash, role, active, \
                invite_token_hash, invite_expires_at \
           FROM users WHERE id = $1",
    )
    .bind(session.user_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| ApiError::Internal(e.into()))?
    .ok_or(ApiError::Unauthorized)?;

    if !user.active {
        return Err(ApiError::Unauthorized);
    }

    let tokens = issue_token_pair(&state, user.id, user.workspace_id, user.role).await?;
    Ok(Json(tokens))
}

#[utoipa::path(
    post,
    path = "/auth/logout",
    request_body = LogoutRequest,
    responses((status = 204, description = "logged out")),
    tag = "auth"
)]
pub async fn logout(
    State(state): State<Arc<AppState>>,
    Json(body): Json<LogoutRequest>,
) -> Result<StatusCode, ApiError> {
    let hash = tokens::sha256_hex(&body.refresh_token);
    sessions::delete_by_token_hash(&state.pool, &hash)
        .await
        .map_err(ApiError::Internal)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/auth/activate",
    request_body = ActivateRequest,
    responses(
        (status = 204, description = "activated"),
        (status = 400, description = "validation error"),
        (status = 401, description = "invite token invalid or expired"),
    ),
    tag = "auth"
)]
pub async fn activate(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ActivateRequest>,
) -> Result<StatusCode, ApiError> {
    body.validate()
        .map_err(|e| ApiError::Validation(e.to_string()))?;

    let invite_hash = tokens::sha256_hex(&body.invite_token);
    let user = users::find_by_invite_token_hash(&state.pool, &invite_hash)
        .await
        .map_err(ApiError::Internal)?
        .ok_or(ApiError::Unauthorized)?;

    let expires_at = user.invite_expires_at.ok_or(ApiError::Unauthorized)?;
    if expires_at <= Utc::now() {
        return Err(ApiError::Unauthorized);
    }

    let password_hash = password::hash(&body.password).map_err(ApiError::Internal)?;

    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;
    users::activate(&mut tx, user.id, &password_hash)
        .await
        .map_err(ApiError::Internal)?;
    tx.commit()
        .await
        .map_err(|e| ApiError::Internal(e.into()))?;

    Ok(StatusCode::NO_CONTENT)
}
