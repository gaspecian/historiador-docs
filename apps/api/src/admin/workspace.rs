//! Admin workspace config and MCP token management endpoints.

use std::sync::Arc;

use axum::{extract::State, Json};
use historiador_db::postgres::{users::Role, workspaces};
use serde::Serialize;
use uuid::Uuid;

use crate::auth::{extractor::AuthUser, rbac::require_role, tokens};
use crate::error::ApiError;
use crate::state::AppState;

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct WorkspaceResponse {
    pub id: Uuid,
    pub name: String,
    pub languages: Vec<String>,
    pub primary_language: String,
    pub llm_provider: String,
    /// The MCP endpoint URL (constructed from config).
    pub mcp_endpoint_url: String,
    /// Whether a bearer token has been configured.
    pub has_mcp_token: bool,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct RegenerateTokenResponse {
    /// The new bearer token in plaintext. This is the only time it
    /// will be visible — the server stores only the sha256 hash.
    pub bearer_token: String,
}

#[utoipa::path(
    get,
    path = "/admin/workspace",
    responses(
        (status = 200, description = "workspace config", body = WorkspaceResponse),
        (status = 401, description = "unauthenticated"),
        (status = 403, description = "caller is not admin"),
        (status = 404, description = "workspace not found"),
    ),
    tag = "admin",
    security(("bearer" = []))
)]
pub async fn get_workspace(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> Result<Json<WorkspaceResponse>, ApiError> {
    require_role(&auth, Role::Admin)?;

    let ws = workspaces::find_by_id(&state.pool, auth.workspace_id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;

    let mcp_endpoint_url = format!(
        "{}:{}",
        state
            .public_base_url
            .trim_end_matches('/')
            .replace(":3000", "")
            .replace(":3001", ""),
        std::env::var("MCP_PORT").unwrap_or_else(|_| "3002".into())
    );

    Ok(Json(WorkspaceResponse {
        id: ws.id,
        name: ws.name,
        languages: ws.languages,
        primary_language: ws.primary_language,
        llm_provider: ws.llm_provider,
        mcp_endpoint_url: format!("{mcp_endpoint_url}/query"),
        has_mcp_token: ws.mcp_bearer_token_hash.is_some(),
    }))
}

#[utoipa::path(
    post,
    path = "/admin/workspace/regenerate-token",
    responses(
        (status = 200, description = "new token generated", body = RegenerateTokenResponse),
        (status = 401, description = "unauthenticated"),
        (status = 403, description = "caller is not admin"),
    ),
    tag = "admin",
    security(("bearer" = []))
)]
pub async fn regenerate_token(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> Result<Json<RegenerateTokenResponse>, ApiError> {
    require_role(&auth, Role::Admin)?;

    let (plaintext, hash) = tokens::generate();

    workspaces::update_mcp_token(&state.pool, auth.workspace_id, &hash)
        .await
        .map_err(ApiError::Internal)?;

    Ok(Json(RegenerateTokenResponse {
        bearer_token: plaintext,
    }))
}
