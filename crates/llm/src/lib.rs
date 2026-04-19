//! `historiador_llm` — LLM provider abstraction.
//!
//! Defines two core traits:
//! - [`EmbeddingClient`] for generating text embeddings
//! - [`TextGenerationClient`] for LLM text completion
//!
//! Provider implementations:
//! - [`openai`] — OpenAI embeddings + text generation via `async-openai`
//! - [`anthropic`] — Anthropic text generation via raw `reqwest`
//! - [`stub`] — Zero-cost stubs for testing

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod anthropic;
pub mod ollama;
pub mod openai;
pub mod stub;
pub mod text_generation;
pub mod tool_calling;

// Re-export the most-used types at crate root for convenience.
pub use anthropic::AnthropicTextGenerationClient;
pub use ollama::{list_models as list_ollama_models, OllamaEmbeddingClient, OllamaTextClient};
pub use openai::{OpenAiEmbeddingClient, OpenAiTextGenerationClient};
pub use stub::{StubEmbeddingClient, StubTextGenerationClient, StubToolCallingClient};
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

/// A single embedding vector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Embedding {
    pub vector: Vec<f32>,
}

/// Trait for generating text embeddings. The API server resolves the
/// concrete implementation from the workspace's `llm_provider` setting.
#[async_trait]
pub trait EmbeddingClient: Send + Sync {
    /// Generate embeddings for a batch of text inputs.
    async fn embed(&self, texts: &[String]) -> Result<Vec<Embedding>, LlmError>;

    /// Dimension of the embedding vectors produced by this client.
    fn dimension(&self) -> usize;
}
