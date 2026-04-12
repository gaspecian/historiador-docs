//! `/editor` HTTP handlers — AI-assisted document drafting.

use std::sync::Arc;

use axum::{extract::State, Json};
use historiador_db::postgres::users::Role;
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::auth::{extractor::AuthUser, rbac::require_role};
use crate::error::ApiError;
use crate::state::AppState;

use super::prompts;

// ---- DTOs ----

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct DraftRequest {
    /// Natural language description of the document to create.
    #[validate(length(min = 10, max = 5000))]
    pub brief: String,
    /// Optional BCP 47 language tag for the output language.
    #[validate(length(min = 2, max = 35))]
    pub language: Option<String>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct DraftResponse {
    pub content_markdown: String,
}

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct IterateRequest {
    /// The current draft markdown to refine.
    #[validate(length(min = 1, max = 50000))]
    pub current_draft: String,
    /// Follow-up instruction describing what to change.
    #[validate(length(min = 1, max = 5000))]
    pub instruction: String,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct IterateResponse {
    pub content_markdown: String,
}

// ---- handlers ----

#[utoipa::path(
    post,
    path = "/editor/draft",
    request_body = DraftRequest,
    responses(
        (status = 200, description = "AI-generated draft", body = DraftResponse),
        (status = 400, description = "validation error"),
        (status = 401, description = "unauthorized"),
        (status = 403, description = "forbidden"),
    ),
    security(("bearer" = [])),
    tag = "editor"
)]
pub async fn draft(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(body): Json<DraftRequest>,
) -> Result<Json<DraftResponse>, ApiError> {
    require_role(&auth, Role::Author)?;
    body.validate()
        .map_err(|e| ApiError::Validation(e.to_string()))?;

    let user_prompt = match &body.language {
        Some(lang) => format!("Write in {lang}.\n\n{}", body.brief),
        None => body.brief.clone(),
    };

    let content_markdown = state
        .text_generation_client
        .generate_text(prompts::DRAFT_SYSTEM_PROMPT, &user_prompt)
        .await
        .map_err(|e| anyhow::anyhow!("LLM error: {e}"))?;

    Ok(Json(DraftResponse { content_markdown }))
}

#[utoipa::path(
    post,
    path = "/editor/iterate",
    request_body = IterateRequest,
    responses(
        (status = 200, description = "updated draft", body = IterateResponse),
        (status = 400, description = "validation error"),
        (status = 401, description = "unauthorized"),
        (status = 403, description = "forbidden"),
    ),
    security(("bearer" = [])),
    tag = "editor"
)]
pub async fn iterate(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Json(body): Json<IterateRequest>,
) -> Result<Json<IterateResponse>, ApiError> {
    require_role(&auth, Role::Author)?;
    body.validate()
        .map_err(|e| ApiError::Validation(e.to_string()))?;

    let user_prompt = format!(
        "## Current Draft\n\n{}\n\n## Instruction\n\n{}",
        body.current_draft, body.instruction
    );

    let content_markdown = state
        .text_generation_client
        .generate_text(prompts::ITERATE_SYSTEM_PROMPT, &user_prompt)
        .await
        .map_err(|e| anyhow::anyhow!("LLM error: {e}"))?;

    Ok(Json(IterateResponse { content_markdown }))
}
