//! Editor-v2 WebSocket handler (Sprint 11, phase A3).
//!
//! Replaces Sprint 4's SSE + POST split with a single authenticated
//! WebSocket per editor tab, keyed by `(page_id, language,
//! author_id)`. Every message carries a monotonic `seq` assigned by
//! the server when the envelope is persisted; on reconnect the
//! client replays missed messages from the `editor_conversations`
//! row for the triple.
//!
//! ## Envelope variants
//!
//! This phase ships the structural variants only: `hello`, `message`,
//! `ack`, `error`. Block-op / tool-call / autonomy / comment variants
//! land in later phases (A4, A10, A11, B1). Unknown variants are
//! dropped per ADR-012 §135–142 — the handshake never errors on an
//! unrecognised message so protocol evolution stays safe.
//!
//! ## Authentication
//!
//! Browsers cannot attach an `Authorization` header to the WebSocket
//! upgrade request, so we accept the access token as a query-string
//! parameter: `?token=<jwt>`. Tokens should be short-lived (minutes,
//! not days) to bound the exposure window if one leaks into a proxy
//! log. A signed-ticket flow is a candidate for a later hardening
//! phase.

use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::Response;
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::time::{interval, Duration};
use uuid::Uuid;

use crate::application::editor::context::{assemble, ContextInputs, HistoryTurn, OutlineEntry};
use crate::application::editor::intake::{determine_mode, IntakeState};
use crate::application::editor::outline::{latest_approved_outline, to_context_entries};
use crate::application::editor::prompt_template::{render_prompt, PromptMode};
use crate::infrastructure::auth::jwt;
use crate::state::AppState;
use historiador_tools::block_op_tools;

/// Protocol version. Bumped when the envelope gains a breaking change
/// (e.g., a renamed required field). Additions that follow the
/// "unknown variants are dropped" rule do not require a bump.
pub const PROTOCOL_VERSION: &str = "2026-04-ws-v1";

/// Heartbeat interval. Kept short enough to detect a dead connection
/// within ~30 s on a web client with the default pong timeout.
const HEARTBEAT: Duration = Duration::from_secs(15);

/// Variants the server advertises in its `hello` handshake. Clients
/// MUST tolerate receiving envelope types not in this list (forward
/// compatibility); they MAY skip rendering them.
pub fn supported_variants() -> Vec<&'static str> {
    vec![
        "hello",
        "client_hello",
        "message",
        "ack",
        "error",
        "tool_call",
        "block_op",
        "block_op_ack",
        "skip_discovery",
        "outline_proposed",
        "outline_revised",
        "outline_approved",
        "autonomy_mode_changed",
        "autonomy_checkpoint",
        "autonomy_decision",
        "comment_posted",
        "comment_resolved",
        "review_requested",
    ]
}

// --- envelope types ---

/// A single envelope over the WebSocket. Tagged union; the `type`
/// field discriminates variants. Every server-originated variant
/// carries a `seq` assigned by the server; client-originated
/// variants omit it and let the server stamp.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EditorMessage {
    /// Sent by the server immediately after the upgrade completes.
    /// Carries the server's view of the last persisted `seq`, the
    /// protocol version, and the variants this build supports.
    Hello {
        protocol_version: String,
        supported_variants: Vec<String>,
        server_last_seq: u64,
    },
    /// First message a client sends after receiving `hello`. Carries
    /// the `seq` the client last observed so the server can replay
    /// anything the client missed while disconnected.
    ClientHello { client_last_seq: u64 },
    /// A conversation turn. `role` is "user" or "assistant"; other
    /// values are tolerated (forward-compat) but ignored by the
    /// persistence layer.
    Message {
        seq: u64,
        role: String,
        content: String,
    },
    /// Acknowledges a client message the server has persisted.
    Ack { client_ref: String, seq: u64 },
    /// A server-visible error the client should surface. Not the
    /// same as transport-level errors (protocol violations close the
    /// socket).
    Error { code: String, message: String },
    /// Raw tool call emitted by the model (client-bound, informational
    /// — the server immediately dispatches and emits a `block_op`).
    /// Kept as its own variant so traces can distinguish "the model
    /// asked to edit X" from "the server approved the edit".
    ToolCall {
        seq: u64,
        call_id: String,
        name: String,
        arguments: serde_json::Value,
    },
    /// A proposed or applied block-level mutation. `proposal_id`
    /// correlates with the `block_op_ack` the client sends when the
    /// author approves or rejects.
    BlockOp {
        seq: u64,
        proposal_id: String,
        op: serde_json::Value,
    },
    /// Client → server: the author resolved a proposal (A10).
    BlockOpAck {
        proposal_id: String,
        decision: String,
    },
    /// Client → server: the author clicked "Skip discovery" (A8 /
    /// US-11.01). Records a session flag so subsequent turns do not
    /// gate on intake questions.
    SkipDiscovery,
    /// Server → client: the AI proposed an outline (A9 / ADR-015).
    OutlineProposed {
        seq: u64,
        sections: Vec<crate::application::editor::outline::OutlineSection>,
    },
    /// Server → client: the AI revised an earlier outline in response
    /// to a comment.
    OutlineRevised {
        seq: u64,
        sections: Vec<crate::application::editor::outline::OutlineSection>,
    },
    /// Client → server: the author accepted an outline. Triggers the
    /// seed-canvas dispatch downstream.
    OutlineApproved {
        sections: Vec<crate::application::editor::outline::OutlineSection>,
    },
    /// Either direction: the autonomy mode for this page changed.
    /// Client-originated when the user flips the selector; server-
    /// originated when the workspace default kicks in or another
    /// client changes the mode.
    AutonomyModeChanged {
        mode: crate::application::editor::autonomy::AutonomyMode,
    },
    /// Server → client: the batcher hit a boundary (heading change,
    /// 5-op threshold, or 10-second timeout) and the agent is
    /// waiting for a decision before drafting more.
    AutonomyCheckpoint {
        seq: u64,
        summary: String,
        op_count: u32,
        reason: crate::application::editor::autonomy::CheckpointReason,
    },
    /// Client → server: user's decision at a checkpoint.
    AutonomyDecision { decision: String },
    /// Either direction: a comment was posted.
    CommentPosted {
        seq: u64,
        comment_id: String,
        block_ids: Vec<String>,
        text: String,
    },
    /// Either direction: a comment was resolved.
    CommentResolved { seq: u64, comment_id: String },
    /// Client → server: the user clicked "Review this doc" (B5 /
    /// US-11.11). The next agent turn runs in review mode — it may
    /// only emit `comment_posted` events, never block ops.
    ReviewRequested,
}

// --- query params ---

#[derive(Debug, Deserialize)]
pub struct WsParams {
    pub page_id: Uuid,
    pub language: String,
    pub token: String,
}

// --- handler ---

pub async fn upgrade(
    State(state): State<Arc<AppState>>,
    Query(params): Query<WsParams>,
    ws: WebSocketUpgrade,
) -> Result<Response, StatusCode> {
    if !state.editor_v2_enabled {
        return Err(StatusCode::NOT_FOUND);
    }

    let claims = jwt::decode_token(&params.token, &state.jwt_secret)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let author_id = claims.sub;
    let page_id = params.page_id;
    let language = params.language.clone();

    Ok(ws.on_upgrade(move |socket| async move {
        if let Err(e) = run_socket(state, socket, page_id, language, author_id).await {
            tracing::warn!(error = %e, %page_id, %author_id, "editor ws closed with error");
        }
    }))
}

async fn run_socket(
    state: Arc<AppState>,
    socket: WebSocket,
    page_id: Uuid,
    language: String,
    author_id: Uuid,
) -> anyhow::Result<()> {
    let (mut sender, mut receiver) = socket.split();

    // Send hello immediately so the client can advertise its last_seq.
    let server_last_seq = load_last_seq(&state, page_id, &language, author_id).await;
    let hello = EditorMessage::Hello {
        protocol_version: PROTOCOL_VERSION.to_string(),
        supported_variants: supported_variants().into_iter().map(String::from).collect(),
        server_last_seq,
    };
    sender
        .send(Message::Text(serde_json::to_string(&hello)?))
        .await?;

    let mut heartbeat = interval(HEARTBEAT);
    heartbeat.tick().await; // skip the immediate first tick

    let mut seq = server_last_seq;
    let mut client_synced = false;
    // Session-scoped intake state. Skip-discovery flips the flag so
    // subsequent turns do not gate on the intake questions. Outline
    // approvals and canvas content are re-read from storage on every
    // turn so the LLM always sees fresh state.
    let mut skip_discovery = false;
    let mut review_pass_requested = false;

    loop {
        tokio::select! {
            maybe_frame = receiver.next() => {
                let Some(frame) = maybe_frame else {
                    break;
                };
                let frame = frame?;

                match frame {
                    Message::Text(text) => {
                        let parsed: serde_json::Result<EditorMessage> = serde_json::from_str(&text);
                        let Ok(msg) = parsed else {
                            tracing::debug!(%text, "unknown or malformed editor envelope — dropped");
                            continue;
                        };

                        match msg {
                            EditorMessage::ClientHello { client_last_seq } => {
                                if !client_synced {
                                    replay_from(&state, &mut sender, page_id, &language, author_id, client_last_seq)
                                        .await?;
                                    client_synced = true;
                                }
                            }
                            EditorMessage::SkipDiscovery => {
                                skip_discovery = true;
                                tracing::debug!(%page_id, %author_id, "skip_discovery flipped on");
                            }
                            EditorMessage::ReviewRequested => {
                                review_pass_requested = true;
                                tracing::debug!(%page_id, %author_id, "review_requested");
                            }
                            EditorMessage::Message { role, content, .. } => {
                                // Echo the user's turn with a real seq so the
                                // client can reconcile the optimistic copy.
                                seq += 1;
                                let stored = EditorMessage::Message {
                                    seq,
                                    role: role.clone(),
                                    content: content.clone(),
                                };
                                sender.send(Message::Text(serde_json::to_string(&stored)?))
                                    .await?;
                                persist_message(&state, page_id, &language, author_id, &role, &content).await;

                                // Only user-initiated turns trigger an LLM reply.
                                if role == "user" {
                                    let reply = generate_reply(
                                        &state,
                                        page_id,
                                        &language,
                                        author_id,
                                        &content,
                                        skip_discovery,
                                        review_pass_requested,
                                    )
                                    .await;
                                    review_pass_requested = false;

                                    if let Some(AssistantReply { text, mode }) = reply {
                                        let chat_content = match mode {
                                            PromptMode::Generation => {
                                                // Write the drafted markdown to the
                                                // canvas instead of dumping it in
                                                // chat. The proposal-overlay pipeline
                                                // takes over for per-block tool
                                                // calls; this is the whole-document
                                                // fallback path used while real
                                                // providers still return
                                                // LlmError::NotImplemented for tool
                                                // calling.
                                                write_canvas_draft(&state, page_id, &language, &text).await;
                                                "Rascunho atualizado — veja o canvas.".to_string()
                                            }
                                            _ => text,
                                        };

                                        seq += 1;
                                        let reply_env = EditorMessage::Message {
                                            seq,
                                            role: "assistant".into(),
                                            content: chat_content.clone(),
                                        };
                                        sender
                                            .send(Message::Text(serde_json::to_string(&reply_env)?))
                                            .await?;
                                        persist_message(
                                            &state,
                                            page_id,
                                            &language,
                                            author_id,
                                            "assistant",
                                            &chat_content,
                                        )
                                        .await;
                                    }
                                }
                            }
                            // All other variants are silently ignored for now —
                            // block_op / comment / autonomy dispatch lands in
                            // follow-up phases.
                            _ => {}
                        }
                    }
                    Message::Ping(data) => {
                        sender.send(Message::Pong(data)).await?;
                    }
                    Message::Pong(_) => {}
                    Message::Close(_) => break,
                    Message::Binary(_) => {
                        // Binary frames are not part of the protocol.
                        tracing::debug!("editor ws: binary frame received — ignored");
                    }
                }
            }
            _ = heartbeat.tick() => {
                sender.send(Message::Ping(Vec::new())).await?;
            }
        }
    }

    Ok(())
}

async fn load_last_seq(state: &AppState, page_id: Uuid, language: &str, author_id: Uuid) -> u64 {
    let res = historiador_db::postgres::editor_conversations::find_by_key(
        &state.pool,
        page_id,
        language,
        author_id,
    )
    .await;
    match res {
        Ok(Some(row)) => row.messages.as_array().map(|a| a.len() as u64).unwrap_or(0),
        _ => 0,
    }
}

async fn replay_from(
    state: &AppState,
    sender: &mut futures::stream::SplitSink<WebSocket, Message>,
    page_id: Uuid,
    language: &str,
    author_id: Uuid,
    client_last_seq: u64,
) -> anyhow::Result<()> {
    let Ok(Some(row)) = historiador_db::postgres::editor_conversations::find_by_key(
        &state.pool,
        page_id,
        language,
        author_id,
    )
    .await
    else {
        return Ok(());
    };

    let Some(messages) = row.messages.as_array() else {
        return Ok(());
    };

    for (i, entry) in messages.iter().enumerate() {
        let seq = (i + 1) as u64;
        if seq <= client_last_seq {
            continue;
        }
        let role = entry
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or("assistant")
            .to_string();
        let content = entry
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let replay = EditorMessage::Message { seq, role, content };
        sender
            .send(Message::Text(serde_json::to_string(&replay)?))
            .await?;
    }
    Ok(())
}

async fn persist_message(
    state: &AppState,
    page_id: Uuid,
    language: &str,
    author_id: Uuid,
    role: &str,
    content: &str,
) {
    // Read-modify-write on the JSONB column. Concurrency is bounded
    // to "one live WS per author per page" by the client; a
    // duplicated tab would race, which is acceptable for this first
    // cut (worst case: messages interleave). A later phase moves
    // persistence to the Chronik topic (ADR-015 / ADR-012).
    let existing = historiador_db::postgres::editor_conversations::find_by_key(
        &state.pool,
        page_id,
        language,
        author_id,
    )
    .await;

    let mut messages = match existing {
        Ok(Some(row)) => row.messages.as_array().cloned().unwrap_or_default(),
        _ => Vec::new(),
    };

    messages.push(serde_json::json!({
        "role": role,
        "content": content,
        "ts": chrono::Utc::now().to_rfc3339(),
    }));

    let json = serde_json::Value::Array(messages);
    if let Err(e) = historiador_db::postgres::editor_conversations::upsert(
        &state.pool,
        page_id,
        language,
        author_id,
        &json,
    )
    .await
    {
        tracing::warn!(error = %e, %page_id, %author_id, "editor ws: persist failed");
    }
}

struct AssistantReply {
    text: String,
    mode: PromptMode,
}

/// Build a full rendered system prompt for the next turn and call
/// the text-generation client to produce an assistant reply.
/// Returns `None` on LLM failure so the caller does not fabricate a
/// response — the user sees a transport error via the client's
/// existing error handling instead of silently-dropped data.
async fn generate_reply(
    state: &AppState,
    page_id: Uuid,
    language: &str,
    author_id: Uuid,
    user_content: &str,
    skip_discovery: bool,
    review_pass: bool,
) -> Option<AssistantReply> {
    // Fetch canvas markdown for context. Missing page = empty canvas.
    let page_markdown = historiador_db::postgres::page_versions::find_by_page_and_language(
        &state.pool,
        page_id,
        language,
    )
    .await
    .ok()
    .flatten()
    .map(|v| v.content_markdown)
    .unwrap_or_default();

    // Pull the transcript so we can hand the LLM recent history AND
    // derive the outline state.
    let transcript = historiador_db::postgres::editor_conversations::find_by_key(
        &state.pool,
        page_id,
        language,
        author_id,
    )
    .await
    .ok()
    .flatten()
    .map(|row| row.messages)
    .unwrap_or_else(|| serde_json::Value::Array(Vec::new()));

    let outline_sections = latest_approved_outline(&transcript);
    let outline_approved = outline_sections.is_some();
    let outline_entries: Vec<OutlineEntry> = outline_sections
        .as_deref()
        .map(to_context_entries)
        .unwrap_or_default();

    let recent_history: Vec<HistoryTurn> = transcript
        .as_array()
        .map(|array| {
            array
                .iter()
                .filter_map(|entry| {
                    let role = entry.get("role")?.as_str()?.to_string();
                    let content = entry.get("content")?.as_str()?.to_string();
                    Some(HistoryTurn { role, content })
                })
                .collect()
        })
        .unwrap_or_default();

    let canvas_has_content = !page_markdown.trim().is_empty();

    let intake_state = IntakeState {
        canvas_has_content,
        outline_approved,
        skip_discovery,
    };

    // Review overrides normal intake/conversation/generation gating.
    let mode = if review_pass {
        PromptMode::Review
    } else {
        determine_mode(intake_state, /* user_requested_generation = */ false)
    };

    let context = assemble(ContextInputs {
        selection_text: None,
        cursor_block_id: None,
        canvas_markdown: page_markdown,
        outline: outline_entries,
        recent_history,
    });

    let tools = block_op_tools();
    let rendered = render_prompt(&state.agent_prompt.body, mode, &tools, &context.text);

    tracing::debug!(
        bytes = rendered.len(),
        mode = ?mode,
        context_bytes = context.bytes,
        "editor ws: calling LLM"
    );

    match state
        .text_generation_client
        .generate_text_stream(&rendered, user_content)
        .await
    {
        Ok(mut stream) => {
            let mut buf = String::new();
            while let Some(chunk) = stream.next().await {
                match chunk {
                    Ok(text) => buf.push_str(&text),
                    Err(e) => {
                        tracing::warn!(error = %e, "editor ws: LLM stream error");
                        return None;
                    }
                }
            }
            if buf.trim().is_empty() {
                None
            } else {
                Some(AssistantReply { text: buf, mode })
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, "editor ws: LLM call failed");
            None
        }
    }
}

/// Persist a fresh canvas draft produced in generation mode.
///
/// This is a whole-document overwrite on the legacy-prose fallback
/// path — free-form LLMs do not (yet) return structured tool calls,
/// so we skip the ADR-013 per-block overlay and write straight to
/// `page_versions.content_markdown`. When a provider starts honouring
/// `ToolCallingClient::generate_with_tools`, the block-op dispatcher
/// takes over and this function stops being called.
async fn write_canvas_draft(state: &AppState, page_id: Uuid, language: &str, markdown: &str) {
    // Direct UPDATE on the existing (page_id, language) row. This
    // path does NOT create a new version: if the author has not
    // opened the page through the normal CRUD flow there is no
    // row to update, and the write is a no-op. The v2 editor shell
    // always binds to an existing page via `/editor?page_id=...`,
    // so the row exists in practice.
    let res = sqlx::query(
        "UPDATE page_versions SET content_markdown = $1, updated_at = now() \
         WHERE page_id = $2 AND language = $3",
    )
    .bind(markdown)
    .bind(page_id)
    .bind(language)
    .execute(&state.pool)
    .await;
    match res {
        Ok(r) if r.rows_affected() == 0 => {
            tracing::warn!(
                %page_id, language,
                "editor ws: canvas draft write found no matching page_version row"
            );
        }
        Err(e) => {
            tracing::warn!(error = %e, %page_id, "editor ws: canvas draft write failed");
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_round_trips_hello() {
        let msg = EditorMessage::Hello {
            protocol_version: PROTOCOL_VERSION.to_string(),
            supported_variants: vec!["hello".into(), "message".into()],
            server_last_seq: 7,
        };
        let s = serde_json::to_string(&msg).unwrap();
        let back: EditorMessage = serde_json::from_str(&s).unwrap();
        assert_eq!(msg, back);
        assert!(s.contains("\"type\":\"hello\""));
    }

    #[test]
    fn envelope_round_trips_message() {
        let msg = EditorMessage::Message {
            seq: 3,
            role: "user".into(),
            content: "hi".into(),
        };
        let s = serde_json::to_string(&msg).unwrap();
        let back: EditorMessage = serde_json::from_str(&s).unwrap();
        assert_eq!(msg, back);
    }

    #[test]
    fn unknown_variant_is_rejected_as_error_not_panic() {
        let raw = r#"{"type":"future_variant","payload":{}}"#;
        let parsed: serde_json::Result<EditorMessage> = serde_json::from_str(raw);
        assert!(
            parsed.is_err(),
            "unknown variants must fail deserialisation so the caller can drop them"
        );
    }

    #[test]
    fn supported_variants_lists_protocol_structural_variants() {
        let list = supported_variants();
        assert!(list.contains(&"hello"));
        assert!(list.contains(&"client_hello"));
        assert!(list.contains(&"message"));
        assert!(list.contains(&"ack"));
        assert!(list.contains(&"error"));
    }
}
