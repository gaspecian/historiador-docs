//! `historiador_llm` — LLM provider abstraction.
//!
//! Defines the [`TextGenerationClient`] trait for LLM text completion.
//!
//! Provider implementations:
//! - [`openai`] — OpenAI text generation via `async-openai`
//! - [`anthropic`] — Anthropic text generation via raw `reqwest`
//! - [`ollama`] — Local Ollama text generation
//! - [`stub`] — Zero-cost stubs for testing
//!
//! Embedding is no longer handled by this crate: Chronik-Stream
//! computes embeddings server-side using the topic's configured
//! model, so the application no longer constructs an embedding
//! client.

use thiserror::Error;

pub mod anthropic;
pub mod ollama;
pub mod openai;
pub mod openai_compat;
pub mod stub;
pub mod text_generation;
pub mod tool_calling;

// Re-export the most-used types at crate root for convenience.
pub use anthropic::AnthropicTextGenerationClient;
pub use ollama::{list_models as list_ollama_models, OllamaTextClient};
pub use openai::{OpenAiGenerationConfig, OpenAiTextGenerationClient};
pub use openai_compat::OpenAiCompatConfig;
pub use stub::{StubTextGenerationClient, StubToolCallingClient};
pub use text_generation::{TextGenerationClient, TextStream};
pub use tool_calling::{ToolCallChunk, ToolCallingClient, ToolStream, ToolStreamItem, Turn};

#[derive(Debug, Error)]
pub enum LlmError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("api error: {message}")]
    Api { message: String },

    #[error("not implemented")]
    NotImplemented,
}
