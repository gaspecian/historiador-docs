//! `/setup/{init,probe,ollama-models}` HTTP handlers — thin Clean
//! Architecture wrappers over [`crate::application::setup`].

use std::sync::atomic::Ordering;
use std::sync::Arc;

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::application::setup::InitializeInstallationCommand;
use crate::error::ApiError;
use crate::setup::llm_probe::LlmProvider;
use crate::state::AppState;

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct SetupRequest {
    #[validate(email)]
    pub admin_email: String,

    #[validate(length(min = 12, max = 256))]
    pub admin_password: String,

    #[validate(length(min = 1, max = 100))]
    pub workspace_name: String,

    pub llm_provider: LlmProvider,

    #[validate(length(min = 1, max = 512))]
    pub llm_api_key: String,

    /// Model used for AI text generation (chat / editor). Optional for
    /// cloud providers (falls back to sensible defaults); required for
    /// Ollama because models are user-managed local pulls.
    #[validate(length(min = 1, max = 128))]
    pub generation_model: Option<String>,

    /// Model used for chunk embeddings during publish.
    #[validate(length(min = 1, max = 128))]
    pub embedding_model: Option<String>,

    #[validate(length(min = 1, max = 16))]
    pub languages: Vec<String>,

    #[validate(length(min = 2, max = 32))]
    pub primary_language: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct SetupResponse {
    pub workspace_id: Uuid,
    pub user_id: Uuid,
    pub setup_complete: bool,
}

#[utoipa::path(
    post,
    path = "/setup/init",
    request_body = SetupRequest,
    responses(
        (status = 200, description = "installation initialized", body = SetupResponse),
        (status = 400, description = "validation error or LLM key rejected"),
        (status = 409, description = "setup already complete"),
    ),
    tag = "setup"
)]
pub async fn init(
    State(state): State<Arc<AppState>>,
    Json(body): Json<SetupRequest>,
) -> Result<Json<SetupResponse>, ApiError> {
    body.validate()
        .map_err(|e| ApiError::Validation(e.to_string()))?;

    let result = state
        .use_cases
        .initialize_installation
        .execute(InitializeInstallationCommand {
            admin_email: body.admin_email,
            admin_password: body.admin_password,
            workspace_name: body.workspace_name,
            llm_provider: body.llm_provider,
            llm_api_key: body.llm_api_key,
            generation_model: body.generation_model,
            embedding_model: body.embedding_model,
            languages: body.languages,
            primary_language: body.primary_language,
        })
        .await?;

    // Flip the cached setup-complete flag so the gate middleware
    // stops returning 423. Presentation concern — not the use case's.
    state.setup_complete.store(true, Ordering::Release);

    Ok(Json(SetupResponse {
        workspace_id: result.workspace_id,
        user_id: result.user_id,
        setup_complete: true,
    }))
}

// ---- probe (test connection without completing setup) ----

#[derive(Debug, Deserialize, utoipa::ToSchema)]
pub struct ProbeRequest {
    pub llm_provider: LlmProvider,
    #[serde(default)]
    pub llm_api_key: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ProbeResponse {
    pub success: bool,
    pub message: String,
}

#[utoipa::path(
    post,
    path = "/setup/probe",
    request_body = ProbeRequest,
    responses(
        (status = 200, description = "probe result", body = ProbeResponse),
    ),
    tag = "setup"
)]
pub async fn probe(
    State(state): State<Arc<AppState>>,
    Json(body): Json<ProbeRequest>,
) -> Result<Json<ProbeResponse>, ApiError> {
    let result = state
        .use_cases
        .probe_llm
        .execute(body.llm_provider, &body.llm_api_key)
        .await?;
    Ok(Json(ProbeResponse {
        success: result.success,
        message: result.message,
    }))
}

// ---- list Ollama models ----

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct OllamaModelsRequest {
    /// Base URL of a reachable Ollama server (e.g. `http://localhost:11434`).
    #[validate(url)]
    pub base_url: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct OllamaModelEntry {
    pub name: String,
    pub size_bytes: u64,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct OllamaModelsResponse {
    pub models: Vec<OllamaModelEntry>,
}

#[utoipa::path(
    post,
    path = "/setup/ollama-models",
    request_body = OllamaModelsRequest,
    responses(
        (status = 200, description = "available models", body = OllamaModelsResponse),
        (status = 400, description = "invalid URL or Ollama unreachable"),
    ),
    tag = "setup"
)]
pub async fn ollama_models(
    State(state): State<Arc<AppState>>,
    Json(body): Json<OllamaModelsRequest>,
) -> Result<Json<OllamaModelsResponse>, ApiError> {
    body.validate()
        .map_err(|e| ApiError::Validation(e.to_string()))?;

    let models = state
        .use_cases
        .list_ollama_models
        .execute(&body.base_url)
        .await?;

    Ok(Json(OllamaModelsResponse {
        models: models
            .into_iter()
            .map(|m| OllamaModelEntry {
                name: m.name,
                size_bytes: m.size_bytes,
            })
            .collect(),
    }))
}
