//! Stub implementations for testing and development.
//!
//! `StubEmbeddingClient` returns zero vectors; `StubTextGenerationClient`
//! returns valid markdown with headings so downstream tests can exercise
//! the full pipeline without hitting a real LLM.

use async_trait::async_trait;

use crate::text_generation::TextGenerationClient;
use crate::{Embedding, EmbeddingClient, LlmError};

/// Stub embedding client that returns zero vectors. Used when no real
/// LLM provider is configured (e.g., provider = "test") or during
/// integration tests.
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

/// Stub text generation client. Returns markdown with headings so the
/// editor → chunker pipeline can be tested end-to-end.
pub struct StubTextGenerationClient;

#[async_trait]
impl TextGenerationClient for StubTextGenerationClient {
    async fn generate_text(
        &self,
        _system_prompt: &str,
        user_prompt: &str,
    ) -> Result<String, LlmError> {
        Ok(format!(
            "## Overview\n\n{user_prompt}\n\n## Details\n\nThis is a stub response for testing.\n"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn stub_embedding_returns_correct_count_and_dimension() {
        let client = StubEmbeddingClient::default();
        let texts = vec!["hello".into(), "world".into()];
        let embeddings = client.embed(&texts).await.unwrap();
        assert_eq!(embeddings.len(), 2);
        assert_eq!(embeddings[0].vector.len(), 1536);
        assert_eq!(embeddings[1].vector.len(), 1536);
    }

    #[tokio::test]
    async fn stub_embedding_empty_input_returns_empty() {
        let client = StubEmbeddingClient::default();
        let embeddings = client.embed(&[]).await.unwrap();
        assert!(embeddings.is_empty());
    }

    #[test]
    fn stub_embedding_dimension_matches_configured_value() {
        let client = StubEmbeddingClient { dim: 768 };
        assert_eq!(client.dimension(), 768);
    }

    #[tokio::test]
    async fn stub_text_generation_returns_markdown_with_headings() {
        let client = StubTextGenerationClient;
        let result = client
            .generate_text("system", "Write about APIs")
            .await
            .unwrap();
        assert!(result.contains("## Overview"));
        assert!(result.contains("## Details"));
        assert!(result.contains("Write about APIs"));
    }
}
