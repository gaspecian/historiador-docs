//! `/editor` HTTP handlers — AI-assisted document drafting.
//!
//! Both endpoints stream the generated markdown as Server-Sent Events.
//! The use case builds the prompt and returns a `TextStream`; the
//! handler wraps it in SSE and emits a telemetry event after the
//! stream ends.
//!
//! Event shape (unchanged from pre-refactor):
//! - `delta` — `data: {"text": "..."}`
//! - `error` — `data: {"message": "..."}`
//! - `done`  — `data: {"length": N}`

use std::convert::Infallible;
use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    response::sse::{Event, KeepAlive, Sse},
    Json,
};
use chrono::{DateTime, Utc};
use futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::application::editor::{
    GenerateDraftCommand, IterateDraftCommand, SaveEditorConversationCommand,
};
use crate::domain::entity::EditorConversationMessage;
use crate::domain::port::event_producer::{DomainEvent, EventProducer};
use crate::domain::value::Language;
use crate::presentation::error::ApiError;
use crate::presentation::extractor::AuthUser;
use crate::state::AppState;

// ---- DTOs ----

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct DraftRequest {
    #[validate(length(min = 10, max = 5000))]
    pub brief: String,
    #[validate(length(min = 2, max = 35))]
    pub language: Option<String>,
}

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct IterateRequest {
    #[validate(length(min = 1, max = 50000))]
    pub current_draft: String,
    #[validate(length(min = 1, max = 5000))]
    pub instruction: String,
}

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
    body.validate()
        .map_err(|e| ApiError::Validation(e.to_string()))?;

    let prepared = state
        .use_cases
        .generate_draft
        .execute(
            auth.as_actor(),
            GenerateDraftCommand {
                brief: body.brief,
                language: body.language,
            },
        )
        .await?;

    let chronik = state.chronik.clone();
    let workspace_id = prepared.workspace_id;
    let user_id = prepared.user_id;

    let event_stream = async_stream::stream! {
        let mut buffer = String::new();
        let mut upstream = prepared.stream;
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

        publish_editor_event(
            chronik.as_ref(),
            DomainEvent::EditorDraftGenerated {
                workspace_id,
                user_id,
                prompt_tokens: None,
            },
        );
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
    body.validate()
        .map_err(|e| ApiError::Validation(e.to_string()))?;

    let prepared = state
        .use_cases
        .iterate_draft
        .execute(
            auth.as_actor(),
            IterateDraftCommand {
                current_draft: body.current_draft,
                instruction: body.instruction,
            },
        )
        .await?;

    let chronik = state.chronik.clone();
    let workspace_id = prepared.workspace_id;
    let user_id = prepared.user_id;

    let event_stream = async_stream::stream! {
        let mut buffer = String::new();
        let mut upstream = prepared.stream;
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

        publish_editor_event(
            chronik.as_ref(),
            DomainEvent::EditorDraftGenerated {
                workspace_id,
                user_id,
                prompt_tokens: None,
            },
        );
    };

    let boxed: std::pin::Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send>> =
        Box::pin(event_stream);
    Ok(Sse::new(boxed).keep_alive(KeepAlive::default()))
}

/// Fire-and-forget telemetry event directly via Chronik when present.
/// Bypasses the use-case / port layer because the event is emitted
/// from inside the SSE generator stream, where awaiting async work is
/// awkward and pointless for non-critical telemetry.
fn publish_editor_event(
    chronik: Option<&historiador_db::chronik::ChronikClient>,
    event: DomainEvent,
) {
    let producer = crate::infrastructure::chronik::ChronikEventProducer::new(chronik.cloned());
    tokio::spawn(async move {
        if let Err(e) = producer.publish(event).await {
            tracing::warn!(error = ?e, "editor telemetry publish failed");
        }
    });
}

// ---- editor conversation persistence (Sprint 10 item #4) ----

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub struct ConversationQuery {
    /// BCP 47 language tag identifying which language the conversation
    /// belongs to, since authors can hold parallel threads per language.
    #[param(example = "en")]
    pub language: String,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ConversationMessageDto {
    /// "user" or "assistant".
    pub role: String,
    pub content: String,
    /// Client-side millisecond Unix timestamp.
    pub ts: i64,
}

#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct SaveConversationRequest {
    pub messages: Vec<ConversationMessageDto>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct ConversationResponse {
    pub page_id: Uuid,
    pub language: String,
    pub messages: Vec<ConversationMessageDto>,
    pub updated_at: DateTime<Utc>,
}

fn parse_language(raw: &str) -> Result<Language, ApiError> {
    Language::parse(raw).map_err(ApiError::from)
}

fn to_dto(m: EditorConversationMessage) -> ConversationMessageDto {
    ConversationMessageDto {
        role: m.role,
        content: m.content,
        ts: m.ts,
    }
}

fn from_dto(m: ConversationMessageDto) -> EditorConversationMessage {
    EditorConversationMessage {
        role: m.role,
        content: m.content,
        ts: m.ts,
    }
}

#[utoipa::path(
    get,
    path = "/pages/{id}/editor-conversation",
    params(
        ("id" = Uuid, Path, description = "page id"),
        ConversationQuery,
    ),
    responses(
        (status = 200, description = "Persisted conversation or an empty transcript if none saved yet.", body = ConversationResponse),
        (status = 401, description = "unauthorized"),
        (status = 403, description = "forbidden"),
        (status = 404, description = "page not found"),
    ),
    security(("bearer" = [])),
    tag = "editor"
)]
pub async fn get_conversation(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(page_id): Path<Uuid>,
    Query(params): Query<ConversationQuery>,
) -> Result<Json<ConversationResponse>, ApiError> {
    let language = parse_language(&params.language)?;
    let conversation = state
        .use_cases
        .load_editor_conversation
        .execute(auth.as_actor(), page_id, language.clone())
        .await?;

    let response = match conversation {
        Some(c) => ConversationResponse {
            page_id: c.page_id,
            language: c.language.into_string(),
            messages: c.messages.into_iter().map(to_dto).collect(),
            updated_at: c.updated_at,
        },
        None => ConversationResponse {
            page_id,
            language: language.into_string(),
            messages: Vec::new(),
            updated_at: Utc::now(),
        },
    };
    Ok(Json(response))
}

#[utoipa::path(
    put,
    path = "/pages/{id}/editor-conversation",
    params(
        ("id" = Uuid, Path, description = "page id"),
        ConversationQuery,
    ),
    request_body = SaveConversationRequest,
    responses(
        (status = 200, description = "Updated conversation echo (client uses `updated_at` for the banner).", body = ConversationResponse),
        (status = 400, description = "validation error"),
        (status = 401, description = "unauthorized"),
        (status = 403, description = "forbidden"),
        (status = 404, description = "page not found"),
    ),
    security(("bearer" = [])),
    tag = "editor"
)]
pub async fn put_conversation(
    State(state): State<Arc<AppState>>,
    auth: AuthUser,
    Path(page_id): Path<Uuid>,
    Query(params): Query<ConversationQuery>,
    Json(body): Json<SaveConversationRequest>,
) -> Result<Json<ConversationResponse>, ApiError> {
    let language = parse_language(&params.language)?;
    let messages = body.messages.into_iter().map(from_dto).collect();

    let saved = state
        .use_cases
        .save_editor_conversation
        .execute(
            auth.as_actor(),
            SaveEditorConversationCommand {
                page_id,
                language,
                messages,
            },
        )
        .await?;

    Ok(Json(ConversationResponse {
        page_id: saved.page_id,
        language: saved.language.into_string(),
        messages: saved.messages.into_iter().map(to_dto).collect(),
        updated_at: saved.updated_at,
    }))
}
