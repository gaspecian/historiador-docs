//! Stub implementations for testing and development.
//!
//! `StubEmbeddingClient` returns zero vectors; `StubTextGenerationClient`
//! returns valid markdown with headings so downstream tests can exercise
//! the full pipeline without hitting a real LLM.

use std::sync::Mutex;

use async_trait::async_trait;
use futures::stream;

use crate::text_generation::{TextGenerationClient, TextStream};
use crate::tool_calling::{ToolCallingClient, ToolStream, ToolStreamItem, Turn};
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
    async fn generate_text_stream(
        &self,
        _system_prompt: &str,
        user_prompt: &str,
    ) -> Result<TextStream, LlmError> {
        let full = format!(
            "## Overview\n\n{user_prompt}\n\n## Details\n\nThis is a stub response for testing.\n"
        );
        Ok(Box::pin(stream::once(async move { Ok(full) })))
    }
}

/// Tool-calling stub backed by an internal FIFO of canned responses.
/// Tests push the items they want the "LLM" to emit, then invoke the
/// dispatcher. Defaults to an empty queue, which means every turn
/// yields an empty stream (useful for "no tool call this turn" paths).
#[derive(Default)]
pub struct StubToolCallingClient {
    queue: Mutex<Vec<Vec<ToolStreamItem>>>,
}

impl StubToolCallingClient {
    pub fn new() -> Self {
        Self::default()
    }

    /// Push a canned response for the next `generate_with_tools` call.
    /// Each call consumes the front item.
    pub fn push(&self, items: Vec<ToolStreamItem>) {
        let mut q = self.queue.lock().expect("stub tool queue poisoned");
        q.push(items);
    }
}

#[async_trait]
impl ToolCallingClient for StubToolCallingClient {
    async fn generate_with_tools(
        &self,
        _system_prompt: &str,
        _messages: &[Turn],
        _tools: &[historiador_tools::ToolSpec],
    ) -> Result<ToolStream, LlmError> {
        let mut q = self.queue.lock().expect("stub tool queue poisoned");
        // FIFO pop: drain from index 0 so tests enqueue in natural
        // order.
        let items = if q.is_empty() {
            Vec::new()
        } else {
            q.remove(0)
        };
        let s = stream::iter(items.into_iter().map(Ok::<_, LlmError>));
        Ok(Box::pin(s))
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

    #[tokio::test]
    async fn stub_text_generation_stream_collects_to_same_string() {
        use futures::StreamExt;

        let client = StubTextGenerationClient;
        let blocking = client.generate_text("sys", "topic").await.unwrap();

        let mut stream = client.generate_text_stream("sys", "topic").await.unwrap();
        let mut collected = String::new();
        while let Some(chunk) = stream.next().await {
            collected.push_str(&chunk.unwrap());
        }
        assert_eq!(blocking, collected);
    }

    #[tokio::test]
    async fn stub_tool_calling_drains_queue_in_order() {
        use crate::tool_calling::ToolCallChunk;
        use futures::StreamExt;
        use serde_json::json;

        let client = StubToolCallingClient::new();
        client.push(vec![
            ToolStreamItem::Text("hi".into()),
            ToolStreamItem::ToolCall(ToolCallChunk {
                call_id: "c1".into(),
                name: "insert_block".into(),
                arguments: json!({ "anchor_block_id": "abc" }),
            }),
        ]);
        client.push(vec![ToolStreamItem::Text("second turn".into())]);

        let mut first = client.generate_with_tools("s", &[], &[]).await.unwrap();
        let mut got: Vec<ToolStreamItem> = Vec::new();
        while let Some(item) = first.next().await {
            got.push(item.unwrap());
        }
        assert_eq!(got.len(), 2);
        assert!(matches!(got[0], ToolStreamItem::Text(_)));
        assert!(matches!(got[1], ToolStreamItem::ToolCall(_)));

        let mut second = client.generate_with_tools("s", &[], &[]).await.unwrap();
        let mut got2: Vec<ToolStreamItem> = Vec::new();
        while let Some(item) = second.next().await {
            got2.push(item.unwrap());
        }
        assert_eq!(got2.len(), 1);
        assert!(matches!(&got2[0], ToolStreamItem::Text(t) if t == "second turn"));
    }

    #[tokio::test]
    async fn stub_tool_calling_empty_queue_yields_empty_stream() {
        use futures::StreamExt;

        let client = StubToolCallingClient::new();
        let mut s = client.generate_with_tools("s", &[], &[]).await.unwrap();
        assert!(s.next().await.is_none());
    }
}
