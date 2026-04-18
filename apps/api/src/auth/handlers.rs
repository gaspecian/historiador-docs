//! `/auth/{login,refresh,logout,activate}` HTTP handlers — thin Clean
//! Architecture wrappers over [`crate::application::auth`].

use std::sync::Arc;

use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::application::auth::{ActivateCommand, LoginCommand};
use crate::domain::value::Email;
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

    let issued = state
        .use_cases
        .login
        .execute(LoginCommand {
            email: Email::parse(body.email).map_err(|_| ApiError::Unauthorized)?,
            password: body.password,
        })
        .await
        // Domain `Forbidden` on login means bad credentials — surface
        // 401 rather than 403 so clients show the right prompt.
        .map_err(login_error_to_401)?;

    Ok(Json(TokenResponse {
        access_token: issued.access_token,
        refresh_token: issued.refresh_token,
        expires_in: issued.expires_in_seconds,
    }))
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
    let issued = state
        .use_cases
        .refresh
        .execute(&body.refresh_token)
        .await
        .map_err(login_error_to_401)?;

    Ok(Json(TokenResponse {
        access_token: issued.access_token,
        refresh_token: issued.refresh_token,
        expires_in: issued.expires_in_seconds,
    }))
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
    state.use_cases.logout.execute(&body.refresh_token).await?;
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

    state
        .use_cases
        .activate
        .execute(ActivateCommand {
            invite_token: body.invite_token,
            password: body.password,
        })
        .await
        .map_err(login_error_to_401)?;
    Ok(StatusCode::NO_CONTENT)
}

/// Map a login/refresh/activate `ApplicationError` onto the right
/// `ApiError`. For these endpoints, `DomainError::Forbidden` is the
/// "bad credentials / bad token" signal and should surface as 401
/// (Unauthorized), not 403 (Forbidden).
fn login_error_to_401(err: crate::domain::error::ApplicationError) -> ApiError {
    use crate::domain::error::{ApplicationError, DomainError};
    match err {
        ApplicationError::Domain(DomainError::Forbidden) => ApiError::Unauthorized,
        other => other.into(),
    }
}
