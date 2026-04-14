//! Admin workspace config and MCP token management endpoints.

use std::sync::Arc;

use axum::{extract::State, Json};
use historiador_db::postgres::{page_versions, users::Role, workspaces};
use historiador_llm::{
    EmbeddingClient, OllamaEmbeddingClient, OpenAiEmbeddingClient, StubEmbeddingClient,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::auth::{extractor::AuthUser, rbac::require_role, tokens};
use crate::error::ApiError;
use crate::setup::llm_probe::LlmProvider;
use crate::state::AppState;

// ---- GET /admin/workspace ----

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct WorkspaceResponse {
    pub id: Uuid,
    pub name: String,
    pub languages: Vec<String>,
    pub primary_language: String,
    pub llm_provider: String,
    pub generation_model: String,
    pub embedding_model: String,
    pub llm_base_url: Option<String>,
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
        generation_model: ws.generation_model,
        embedding_model: ws.embedding_model,
        llm_base_url: ws.llm_base_url,
        mcp_endpoint_url: format!("{mcp_endpoint_url}/query"),
        has_mcp_token: ws.mcp_bearer_token_hash.is_some(),
    }))
}

// ---- POST /admin/workspace/regenerate-token ----

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

// ---- PATCH /admin/workspace/llm ----

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct LlmPatchRequest {
    pub llm_provider: LlmProvider,
    /// API key for cloud providers or base URL for Ollama. Leave
    /// empty to keep the existing secret (useful when editing only
    /// the model names).
    #[validate(length(max = 512))]
    #[serde(default)]
    pub llm_api_key: String,
    #[validate(length(min = 1, max = 128))]
    pub generation_model: String,
    #[validate(length(min = 1, max = 128))]
    pub embedding_model: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct LlmPatchResponse {
    pub success: bool,
    /// When the embedding model changed and published chunks exist,
    /// the admin must trigger a re-index — this field tells the UI
    /// how many page versions are affected so it can show a confirm.
    pub requires_reindex: bool,
    pub affected_page_versions: i64,
    /// True when the generation model changed. The live AppState
    /// clients keep the previous config until the API process is
    /// restarted; the UI surfaces this so the admin can plan a
    /// restart.
    pub requires_restart: bool,
}

#[utoipa::path(
    patch,
    path = "/admin/workspace/llm",
    request_body = LlmPatchRequest,
    responses(
        (status = 200, description = "llm config updated", body = LlmPatchResponse),
        (status = 400, description = "validation / probe failed"),
        (status = 401, description = "unauthenticated"),
        (status = 403, description = "caller is not admin"),
    ),
    tag = "admin",
    security(("bearer" = []))
)]
pub async fn update_llm_config(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(body): Json<LlmPatchRequest>,
) -> Result<Json<LlmPatchResponse>, ApiError> {
    require_role(&auth, Role::Admin)?;
    body.validate()
        .map_err(|e| ApiError::Validation(e.to_string()))?;

    let ws = workspaces::find_by_id(&state.pool, auth.workspace_id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;

    // Only probe when the admin is also rotating the secret (empty
    // string ⇒ keep the existing one untouched).
    if !body.llm_api_key.is_empty() {
        state
            .llm_probe
            .probe(body.llm_provider, &body.llm_api_key)
            .await
            .map_err(|e| ApiError::Validation(format!("LLM rejected: {e}")))?;
    }

    // Compute the (encrypted_key, base_url) pair to persist.
    let (encrypted_key, base_url): (Option<String>, Option<String>) = match body.llm_provider {
        LlmProvider::Ollama => {
            if body.llm_api_key.is_empty() {
                (None, ws.llm_base_url.clone())
            } else {
                (None, Some(body.llm_api_key.trim().to_string()))
            }
        }
        LlmProvider::Test => (None, None),
        LlmProvider::OpenAi | LlmProvider::Anthropic => {
            if body.llm_api_key.is_empty() {
                // Keep existing encrypted secret (handled via COALESCE in SQL).
                (None, None)
            } else {
                let encrypted = state
                    .cipher
                    .encrypt(&body.llm_api_key)
                    .map_err(ApiError::Internal)?;
                (Some(encrypted), None)
            }
        }
    };

    workspaces::update_llm_config(
        &state.pool,
        auth.workspace_id,
        workspaces::LlmConfigPatch {
            llm_provider: body.llm_provider.as_db_str(),
            llm_api_key_encrypted: encrypted_key.as_deref(),
            llm_base_url: base_url.as_deref(),
            generation_model: &body.generation_model,
            embedding_model: &body.embedding_model,
        },
    )
    .await
    .map_err(ApiError::Internal)?;

    // Re-index detection: if the embedding model changed AND there are
    // published page versions in this workspace, warn the UI.
    let embedding_changed = body.embedding_model != ws.embedding_model;
    let affected_page_versions = if embedding_changed {
        page_versions::find_all_published_in_workspace(&state.pool, auth.workspace_id)
            .await
            .map_err(ApiError::Internal)?
            .len() as i64
    } else {
        0
    };

    let generation_changed = body.generation_model != ws.generation_model
        || body.llm_provider.as_db_str() != ws.llm_provider;

    Ok(Json(LlmPatchResponse {
        success: true,
        requires_reindex: embedding_changed && affected_page_versions > 0,
        affected_page_versions,
        requires_restart: generation_changed,
    }))
}

// ---- POST /admin/workspace/reindex ----

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ReindexResponse {
    /// How many published page versions were scheduled for re-embedding.
    pub scheduled: i64,
}

#[utoipa::path(
    post,
    path = "/admin/workspace/reindex",
    responses(
        (status = 202, description = "re-indexing spawned", body = ReindexResponse),
        (status = 401, description = "unauthenticated"),
        (status = 403, description = "caller is not admin"),
    ),
    tag = "admin",
    security(("bearer" = []))
)]
pub async fn reindex(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
) -> Result<Json<ReindexResponse>, ApiError> {
    require_role(&auth, Role::Admin)?;

    let ws = workspaces::find_by_id(&state.pool, auth.workspace_id)
        .await
        .map_err(ApiError::Internal)?
        .ok_or(ApiError::NotFound)?;

    let versions = page_versions::find_all_published_in_workspace(&state.pool, auth.workspace_id)
        .await
        .map_err(ApiError::Internal)?;
    let scheduled = versions.len() as i64;

    // Build an embedding client that reflects the *current* workspace
    // config (i.e. the model the admin just picked) — not the one
    // baked into AppState at boot. Run in a detached task so the HTTP
    // response returns immediately.
    let embedding_client: Arc<dyn EmbeddingClient> = match ws.llm_provider.as_str() {
        "ollama" => {
            let base = ws
                .llm_base_url
                .clone()
                .unwrap_or_else(|| "http://localhost:11434".to_string());
            Arc::new(OllamaEmbeddingClient::new(&base, &ws.embedding_model))
        }
        "openai" | "anthropic" => {
            // Decrypt the workspace key; fall back to the live AppState
            // client when no key is stored (Anthropic-with-stub-embed).
            match ws.llm_api_key_encrypted.as_deref() {
                Some(encrypted) => {
                    let key = state
                        .cipher
                        .decrypt(encrypted)
                        .map_err(ApiError::Internal)?;
                    Arc::new(OpenAiEmbeddingClient::with_model(
                        &key,
                        &ws.embedding_model,
                        1536,
                    ))
                }
                None => Arc::new(StubEmbeddingClient::default()),
            }
        }
        _ => Arc::new(StubEmbeddingClient::default()),
    };

    let pool = state.pool.clone();
    let vector_store = state.vector_store.clone();
    tokio::spawn(async move {
        for version in versions {
            if let Err(e) = crate::pages::pipeline::run_chunk_pipeline(
                &pool,
                vector_store.as_ref(),
                embedding_client.as_ref(),
                &version,
            )
            .await
            {
                tracing::error!(
                    page_version_id = %version.id,
                    error = %e,
                    "re-index pipeline failed for version"
                );
            }
        }
        tracing::info!("workspace re-index complete");
    });

    Ok(Json(ReindexResponse { scheduled }))
}
