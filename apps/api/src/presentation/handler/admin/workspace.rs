//! Admin workspace config and MCP token management endpoints — thin
//! Clean Architecture wrappers over [`crate::application::admin`].
//!
//! The reindex endpoint keeps `tokio::spawn` here (not in the use
//! case) because detaching a task from the async runtime is a
//! presentation concern, not a business one.

use std::sync::Arc;

use axum::{extract::State, Json};
use historiador_chunker::{chunk_markdown, ChunkConfig};
use historiador_db::postgres::chunks;
use historiador_db::vector_store::ChunkPayload;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::application::admin::UpdateLlmConfigCommand;
use crate::infrastructure::llm::probe::LlmProvider;
use crate::presentation::error::ApiError;
use crate::presentation::extractor::AuthUser;
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
    let ws = state
        .use_cases
        .get_workspace
        .execute(auth.as_actor())
        .await?;

    let mcp_endpoint_url = build_mcp_url(&state.public_base_url);

    Ok(Json(WorkspaceResponse {
        id: ws.id,
        name: ws.name,
        languages: ws.languages.into_iter().map(|l| l.into_string()).collect(),
        primary_language: ws.primary_language.into_string(),
        llm_provider: ws.llm_provider,
        generation_model: ws.generation_model,
        embedding_model: ws.embedding_model,
        llm_base_url: ws.llm_base_url,
        mcp_endpoint_url: format!("{mcp_endpoint_url}/query"),
        has_mcp_token: ws.mcp_bearer_token_hash.is_some(),
    }))
}

fn build_mcp_url(public_base_url: &str) -> String {
    let stripped = public_base_url
        .trim_end_matches('/')
        .replace(":3000", "")
        .replace(":3001", "");
    let mcp_port = std::env::var("MCP_PORT").unwrap_or_else(|_| "3002".into());
    format!("{stripped}:{mcp_port}")
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
    let plaintext = state
        .use_cases
        .regenerate_token
        .execute(auth.as_actor())
        .await?;
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
    /// the model names) or — for the OpenAI provider with a custom
    /// `base_url` — to mark the endpoint as unauthenticated.
    #[validate(length(max = 512))]
    #[serde(default)]
    pub llm_api_key: String,

    /// Optional OpenAI-compatible base URL (e.g.
    /// `https://litellm.example/v1`). Persisted verbatim into
    /// `workspaces.llm_base_url`. When set together with an empty
    /// `llm_api_key`, no Authorization header is sent.
    #[validate(length(max = 512))]
    #[validate(custom(function = "crate::presentation::validation::validate_llm_base_url"))]
    #[serde(default)]
    pub base_url: Option<String>,

    #[validate(length(min = 1, max = 128))]
    pub generation_model: String,
    #[validate(length(min = 1, max = 128))]
    pub embedding_model: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct LlmPatchResponse {
    pub success: bool,
    pub requires_reindex: bool,
    pub affected_page_versions: i64,
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
    body.validate()
        .map_err(|e| ApiError::Validation(e.to_string()))?;

    let result = state
        .use_cases
        .update_llm_config
        .execute(
            auth.as_actor(),
            UpdateLlmConfigCommand {
                llm_provider: body.llm_provider,
                llm_api_key: body.llm_api_key,
                base_url: body.base_url,
                generation_model: body.generation_model,
            },
        )
        .await?;

    // `embedding_model`, `requires_reindex`, and `affected_page_versions`
    // are retired by Tasks 5-7 of the EmbeddingClient retirement plan.
    // The DTO fields are dropped in Task 7; for now we ignore the
    // request field and surface dummy values in the response.
    let _ = body.embedding_model;
    Ok(Json(LlmPatchResponse {
        success: true,
        requires_reindex: false,
        affected_page_versions: 0,
        requires_restart: result.requires_restart,
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
    let plan = state
        .use_cases
        .reindex_workspace
        .execute(auth.as_actor())
        .await?;
    let scheduled = plan.scheduled();

    // Kick off the re-index in the background. Chronik embeds server-side,
    // so no embedding client is needed here. Keeping tokio::spawn in the
    // handler (rather than the use case) lets the use case stay free of
    // runtime imports.
    let pool = state.pool.clone();
    let vector_store = state.vector_store.clone();
    tokio::spawn(async move {
        for version in plan.versions {
            if let Err(e) = run_reindex(&pool, vector_store.as_ref(), version).await {
                tracing::error!(error = %e, "re-index pipeline failed for version");
            }
        }
        tracing::info!("workspace re-index complete");
    });

    Ok(Json(ReindexResponse { scheduled }))
}

/// Per-version reindex: delete old Postgres chunk rows, chunk the
/// markdown, produce to Chronik, and store the back-pointers. Chronik
/// embeds server-side — no embedding client needed.
///
/// Orphaned Chronik records from the previous index are filtered out
/// by MCP enrichment (no matching Postgres row).
async fn run_reindex(
    pool: &sqlx::PgPool,
    vector_store: &dyn historiador_db::vector_store::VectorStore,
    version: crate::domain::entity::PageVersion,
) -> anyhow::Result<()> {
    let page_version_id = version.id;

    // Full reindex: unconditionally delete existing Postgres rows.
    chunks::delete_by_page_version(pool, page_version_id).await?;

    let config = ChunkConfig::default();
    let raw_chunks = match chunk_markdown(&version.content_markdown, &config) {
        Ok(c) => c,
        Err(historiador_chunker::ChunkError::EmptyInput) => return Ok(()),
    };
    if raw_chunks.is_empty() {
        return Ok(());
    }

    let payloads: Vec<ChunkPayload> = raw_chunks
        .iter()
        .map(|c| ChunkPayload {
            page_version_id: page_version_id.to_string(),
            section_index: c.section_index as i32,
            heading_path: c.heading_path.clone(),
            content: c.content.clone(),
            language: version.language.as_str().to_string(),
            token_count: c.token_count as i32,
        })
        .collect();

    let produced = vector_store
        .produce_chunks(payloads)
        .await
        .map_err(|e| anyhow::anyhow!("chronik produce failed: {e}"))?;

    let new_chunks: Vec<chunks::NewChunk> = raw_chunks
        .iter()
        .zip(produced.iter())
        .map(|(chunk, rec)| chunks::NewChunk {
            page_version_id,
            heading_path: chunk.heading_path.clone(),
            section_index: chunk.section_index as i32,
            token_count: chunk.token_count as i32,
            oversized: chunk.oversized,
            language: version.language.as_str().to_string(),
            chronik_partition: rec.partition,
            chronik_offset: rec.offset,
        })
        .collect();

    chunks::insert_batch(pool, &new_chunks).await?;
    Ok(())
}
