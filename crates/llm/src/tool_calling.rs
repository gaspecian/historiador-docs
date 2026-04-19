//! `ToolCallingClient` — LLM layer extension for Sprint 11's block-op
//! tool calls (ADR-011).
//!
//! The existing `TextGenerationClient` is text-in / text-out.
//! Sprint 11 also needs structured tool calls: the LLM is given a
//! list of `historiador_tools::ToolSpec` definitions and may emit
//! `tool_call` chunks alongside text. Providers buffer tool-call
//! arguments until the JSON is complete before emitting (ADR-011
//! §56) so downstream dispatchers never see a partial object.
//!
//! Implementations that do not yet support tool calling return
//! `LlmError::NotImplemented` and callers fall back to the text-only
//! path.

use async_trait::async_trait;
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};

use crate::LlmError;

/// One turn of conversation fed to the model. Role is "system",
/// "user", or "assistant".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Turn {
    pub role: String,
    pub content: String,
}

/// A chunk emitted by the tool-calling stream. Providers buffer
/// tool-call JSON internally; `ToolCallChunk` is only yielded once
/// the arguments object is complete and parses as valid JSON.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ToolStreamItem {
    Text(String),
    ToolCall(ToolCallChunk),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCallChunk {
    pub call_id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

/// Owned stream of tool-stream items.
pub type ToolStream = BoxStream<'static, Result<ToolStreamItem, LlmError>>;

/// Minimum contract for a provider that can emit tool calls.
/// Deliberately not a super-trait of `TextGenerationClient` so a
/// provider may implement one, the other, or both.
#[async_trait]
pub trait ToolCallingClient: Send + Sync {
    /// Generate with the supplied tools available. Messages are the
    /// conversation history (most recent last). `tools` is the
    /// curated list of specs the model may invoke; passing an empty
    /// slice is allowed and disables tool-calling for the turn.
    async fn generate_with_tools(
        &self,
        system_prompt: &str,
        messages: &[Turn],
        tools: &[historiador_tools::ToolSpec],
    ) -> Result<ToolStream, LlmError>;
}
