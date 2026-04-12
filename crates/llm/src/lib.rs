//! `historiador_llm` — LLM provider abstraction.
//!
//! Defines the `EmbeddingClient` trait for generating text embeddings,
//! plus a `StubEmbeddingClient` that returns zero vectors for testing
//! and for Sprint 3 (before real LLM providers are wired).
//!
//! Real implementations (OpenAI, Anthropic, Ollama) will be added in
//! Sprint 4 when the MCP query path needs actual embeddings.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

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

/// Stub implementation that returns zero vectors. Used when no real
/// LLM provider is configured (e.g., provider = "test") or during
/// integration tests. Sufficient for Sprint 3 where the chunking
/// pipeline must run but real embeddings aren't queried yet.
pub struct StubEmbeddingClient {
    pub dim: usize,
}

impl Default for StubEmbeddingClient {
    fn default() -> Self {
        Self { dim: 1536 } // OpenAI text-embedding-3-small dimension
    }
}

#[async_trait]
impl EmbeddingClient for StubEmbeddingClient {
    async fn embed(&self, texts: &[String]) -> Result<Vec<Embedding>, LlmError> {
        Ok(texts
            .iter()
            .map(|_| Embedding {
                vector: vec![0.0; self.dim],
            })
            .collect())
    }

    fn dimension(&self) -> usize {
        self.dim
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn stub_returns_correct_count_and_dimension() {
        let client = StubEmbeddingClient::default();
        let texts = vec!["hello".into(), "world".into()];
        let embeddings = client.embed(&texts).await.unwrap();
        assert_eq!(embeddings.len(), 2);
        assert_eq!(embeddings[0].vector.len(), 1536);
        assert_eq!(embeddings[1].vector.len(), 1536);
    }

    #[tokio::test]
    async fn stub_empty_input_returns_empty() {
        let client = StubEmbeddingClient::default();
        let embeddings = client.embed(&[]).await.unwrap();
        assert!(embeddings.is_empty());
    }

    #[test]
    fn dimension_matches_configured_value() {
        let client = StubEmbeddingClient { dim: 768 };
        assert_eq!(client.dimension(), 768);
    }
}
