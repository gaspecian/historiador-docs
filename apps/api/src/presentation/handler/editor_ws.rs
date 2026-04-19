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

use crate::infrastructure::auth::jwt;
use crate::state::AppState;

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
                            EditorMessage::Message { role, content, .. } => {
                                seq += 1;
                                let stored = EditorMessage::Message {
                                    seq,
                                    role: role.clone(),
                                    content: content.clone(),
                                };
                                sender.send(Message::Text(serde_json::to_string(&stored)?))
                                    .await?;
                                persist_message(&state, page_id, &language, author_id, &role, &content).await;
                            }
                            // All other variants are silently ignored for now —
                            // they will be handled by later phases.
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
