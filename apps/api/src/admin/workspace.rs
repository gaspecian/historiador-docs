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
use historiador_db::vector_store::ChunkEmbedding;
use historiador_llm::{
    EmbeddingClient, OllamaEmbeddingClient, OpenAiEmbeddingClient, StubEmbeddingClient,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::application::admin::UpdateLlmConfigCommand;
use crate::auth::extractor::AuthUser;
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
                generation_model: body.generation_model,
                embedding_model: body.embedding_model,
            },
        )
        .await?;

    Ok(Json(LlmPatchResponse {
        success: true,
        requires_reindex: result.requires_reindex,
        affected_page_versions: result.affected_page_versions,
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

    // Build an embedding client reflecting the **current** workspace
    // config (not the one baked into AppState at boot) and kick off
    // the re-embed in the background. Keeping this in the handler
    // (rather than the use case) lets the use case stay free of
    // tokio::spawn and LLM-SDK imports.
    let embedding_client: Arc<dyn EmbeddingClient> = match plan.workspace.llm_provider.as_str() {
        "ollama" => {
            let base = plan
                .workspace
                .llm_base_url
                .clone()
                .unwrap_or_else(|| "http://localhost:11434".to_string());
            Arc::new(OllamaEmbeddingClient::new(
                &base,
                &plan.workspace.embedding_model,
            ))
        }
        "openai" | "anthropic" => match plan.workspace.llm_api_key_encrypted.as_deref() {
            Some(encrypted) => {
                let key = state.cipher.decrypt(encrypted).map_err(ApiError::Internal)?;
                Arc::new(OpenAiEmbeddingClient::with_model(
                    &key,
                    &plan.workspace.embedding_model,
                    1536,
                ))
            }
            None => Arc::new(StubEmbeddingClient::default()),
        },
        _ => Arc::new(StubEmbeddingClient::default()),
    };

    let pool = state.pool.clone();
    let vector_store = state.vector_store.clone();
    tokio::spawn(async move {
        for version in plan.versions {
            if let Err(e) = run_reindex(
                &pool,
                vector_store.as_ref(),
                embedding_client.as_ref(),
                version,
            )
            .await
            {
                tracing::error!(error = %e, "re-index pipeline failed for version");
            }
        }
        tracing::info!("workspace re-index complete");
    });

    Ok(Json(ReindexResponse { scheduled }))
}

/// Per-version reindex: delete old chunks from Postgres + vector store,
/// chunk the markdown, generate embeddings, upsert. Inlined here
/// instead of going through the `ChunkPipeline` port because the
/// admin-supplied embedding client is constructed per-call from the
/// live workspace row, not the one baked into AppState.
async fn run_reindex(
    pool: &sqlx::PgPool,
    vector_store: &dyn historiador_db::vector_store::VectorStore,
    embedding_client: &dyn EmbeddingClient,
    version: crate::domain::entity::PageVersion,
) -> anyhow::Result<()> {
    let page_version_id = version.id;
    let existing = chunks::find_by_page_version(pool, page_version_id).await?;
    if !existing.is_empty() {
        vector_store
            .delete_by_page_version(&page_version_id.to_string())
            .await
            .map_err(|e| anyhow::anyhow!("vector store delete failed: {e}"))?;
        chunks::delete_by_page_version(pool, page_version_id).await?;
    }

    let config = ChunkConfig::default();
    let raw_chunks = match chunk_markdown(&version.content_markdown, &config) {
        Ok(c) => c,
        Err(historiador_chunker::ChunkError::EmptyInput) => return Ok(()),
    };
    if raw_chunks.is_empty() {
        return Ok(());
    }

    let texts: Vec<String> = raw_chunks.iter().map(|c| c.content.clone()).collect();
    let embeddings = embedding_client
        .embed(&texts)
        .await
        .map_err(|e| anyhow::anyhow!("embedding failed: {e}"))?;

    let chunk_embeddings: Vec<ChunkEmbedding> = raw_chunks
        .iter()
        .zip(embeddings.iter())
        .map(|(chunk, emb)| ChunkEmbedding {
            page_version_id: page_version_id.to_string(),
            section_index: chunk.section_index as i32,
            heading_path: chunk.heading_path.clone(),
            content: chunk.content.clone(),
            language: version.language.as_str().to_string(),
            token_count: chunk.token_count as i32,
            embedding: emb.vector.clone(),
        })
        .collect();

    let vexfs_refs = vector_store
        .upsert_chunks(chunk_embeddings)
        .await
        .map_err(|e| anyhow::anyhow!("vector store upsert failed: {e}"))?;

    let new_chunks: Vec<chunks::NewChunk> = raw_chunks
        .iter()
        .zip(vexfs_refs.iter())
        .map(|(chunk, vexfs_ref)| chunks::NewChunk {
            page_version_id,
            heading_path: chunk.heading_path.clone(),
            section_index: chunk.section_index as i32,
            token_count: chunk.token_count as i32,
            oversized: chunk.oversized,
            language: version.language.as_str().to_string(),
            vexfs_ref: vexfs_ref.clone(),
        })
        .collect();

    chunks::insert_batch(pool, &new_chunks).await?;
    Ok(())
}
