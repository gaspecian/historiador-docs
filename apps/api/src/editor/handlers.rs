//! `/editor` HTTP handlers — AI-assisted document drafting.
//!
//! Both endpoints stream the generated markdown as Server-Sent Events:
//!
//! - `delta` events (`data: {"text": "..."}`) — one per provider chunk.
//! - Optional `error` event (`data: {"message": "..."}`) — if the
//!   upstream LLM fails mid-stream.
//! - `done` event (`data: {"length": <bytes>}`) — always terminates the
//!   stream on clean completion.
//!
//! Chronik event logging (ADR-007, topic `editor-conversations`) runs
//! after the stream ends so the full response length is captured.

use std::convert::Infallible;
use std::sync::Arc;

use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
    Json,
};
use futures::{Stream, StreamExt};
use historiador_db::postgres::users::Role;
use serde::Deserialize;
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

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct IterateRequest {
    /// The current draft markdown to refine.
    #[validate(length(min = 1, max = 50000))]
    pub current_draft: String,
    /// Follow-up instruction describing what to change.
    #[validate(length(min = 1, max = 5000))]
    pub instruction: String,
}

// ---- handlers ----

type SseResponse = Sse<std::pin::Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>>>;

#[utoipa::path(
    post,
    path = "/editor/draft",
    request_body = DraftRequest,
    responses(
        (
            status = 200,
            description = "SSE stream of generated markdown; \
                event types: `delta` (data: {\"text\": \"...\"}), \
                `error` (data: {\"message\": \"...\"}), \
                `done` (data: {\"length\": N}). Content-Type: text/event-stream.",
            content_type = "text/event-stream"
        ),
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
) -> Result<SseResponse, ApiError> {
    require_role(&auth, Role::Author)?;
    body.validate()
        .map_err(|e| ApiError::Validation(e.to_string()))?;

    let user_prompt = match &body.language {
        Some(lang) => format!("Write in {lang}.\n\n{}", body.brief),
        None => body.brief.clone(),
    };

    let upstream = state
        .text_generation_client
        .generate_text_stream(prompts::DRAFT_SYSTEM_PROMPT, &user_prompt)
        .await
        .map_err(|e| anyhow::anyhow!("LLM error: {e}"))?;

    let chronik = state.chronik.clone();
    let user_id = auth.user_id;
    let brief = body.brief;
    let language = body.language;

    let event_stream = async_stream::stream! {
        let mut buffer = String::new();
        let mut upstream = upstream;
        while let Some(item) = upstream.next().await {
            match item {
                Ok(chunk) => {
                    buffer.push_str(&chunk);
                    let payload = serde_json::json!({"text": chunk}).to_string();
                    yield Ok::<Event, Infallible>(Event::default().event("delta").data(payload));
                }
                Err(e) => {
                    let payload = serde_json::json!({"message": e.to_string()}).to_string();
                    yield Ok(Event::default().event("error").data(payload));
                    break;
                }
            }
        }
        let done = serde_json::json!({"length": buffer.len()}).to_string();
        yield Ok(Event::default().event("done").data(done));

        if let Some(ref chronik) = chronik {
            chronik.produce_event_fire_and_forget(
                historiador_db::chronik::producer::topics::EDITOR_CONVERSATIONS,
                user_id.to_string(),
                serde_json::json!({
                    "type": "draft",
                    "user_id": user_id,
                    "brief": brief,
                    "language": language,
                    "response_length": buffer.len(),
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                }),
            );
        }
    };

    let boxed: std::pin::Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>> =
        Box::pin(event_stream);
    Ok(Sse::new(boxed).keep_alive(KeepAlive::default()))
}

#[utoipa::path(
    post,
    path = "/editor/iterate",
    request_body = IterateRequest,
    responses(
        (
            status = 200,
            description = "SSE stream of refined markdown; see /editor/draft for event shape.",
            content_type = "text/event-stream"
        ),
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
) -> Result<SseResponse, ApiError> {
    require_role(&auth, Role::Author)?;
    body.validate()
        .map_err(|e| ApiError::Validation(e.to_string()))?;

    let user_prompt = format!(
        "## Current Draft\n\n{}\n\n## Instruction\n\n{}",
        body.current_draft, body.instruction
    );

    let upstream = state
        .text_generation_client
        .generate_text_stream(prompts::ITERATE_SYSTEM_PROMPT, &user_prompt)
        .await
        .map_err(|e| anyhow::anyhow!("LLM error: {e}"))?;

    let chronik = state.chronik.clone();
    let user_id = auth.user_id;
    let instruction = body.instruction;

    let event_stream = async_stream::stream! {
        let mut buffer = String::new();
        let mut upstream = upstream;
        while let Some(item) = upstream.next().await {
            match item {
                Ok(chunk) => {
                    buffer.push_str(&chunk);
                    let payload = serde_json::json!({"text": chunk}).to_string();
                    yield Ok::<Event, Infallible>(Event::default().event("delta").data(payload));
                }
                Err(e) => {
                    let payload = serde_json::json!({"message": e.to_string()}).to_string();
                    yield Ok(Event::default().event("error").data(payload));
                    break;
                }
            }
        }
        let done = serde_json::json!({"length": buffer.len()}).to_string();
        yield Ok(Event::default().event("done").data(done));

        if let Some(ref chronik) = chronik {
            chronik.produce_event_fire_and_forget(
                historiador_db::chronik::producer::topics::EDITOR_CONVERSATIONS,
                user_id.to_string(),
                serde_json::json!({
                    "type": "iterate",
                    "user_id": user_id,
                    "instruction": instruction,
                    "response_length": buffer.len(),
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                }),
            );
        }
    };

    let boxed: std::pin::Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>> =
        Box::pin(event_stream);
    Ok(Sse::new(boxed).keep_alive(KeepAlive::default()))
}
