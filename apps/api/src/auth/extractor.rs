//! Axum extractor that pulls the caller's identity out of a Bearer
//! token. Handlers declare `AuthUser` in their parameter list and
//! the extractor handles Authorization header parsing + JWT verify.
//!
//! No database hit on the hot path: the JWT carries the user id,
//! workspace id, and role, so route guards can enforce RBAC without
//! a round-trip. Database lookups happen only on login, refresh,
//! logout, and activation — the actual auth boundary crossings.

use std::sync::Arc;

use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts},
};
use historiador_db::postgres::users::Role;
use uuid::Uuid;

use crate::auth::jwt;
use crate::error::ApiError;
use crate::state::AppState;

/// Authenticated user, derived from a valid access token.
#[derive(Debug, Clone, Copy)]
pub struct AuthUser {
    pub user_id: Uuid,
    pub workspace_id: Uuid,
    pub role: Role,
}

#[async_trait]
impl FromRequestParts<Arc<AppState>> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or(ApiError::Unauthorized)?;

        let token = header
            .strip_prefix("Bearer ")
            .ok_or(ApiError::Unauthorized)?;

        let claims =
            jwt::decode_token(token, &state.jwt_secret).map_err(|_| ApiError::Unauthorized)?;

        Ok(AuthUser {
            user_id: claims.sub,
            workspace_id: claims.wsid,
            role: claims.role,
        })
    }
}
