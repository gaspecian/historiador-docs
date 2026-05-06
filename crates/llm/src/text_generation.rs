//! `TextGenerationClient` trait for LLM text completion.
//!
//! The primitive is streaming: implementations yield text chunks as the
//! upstream provider produces them. A default `generate_text` collects
//! the stream into a single `String` for callers that do not care about
//! incremental output.

use async_trait::async_trait;
use futures::stream::BoxStream;
use futures::StreamExt;

use crate::tool_calling::Turn;
use crate::LlmError;

/// Owned stream of text chunks. Implementations return `Box::pin(...)`.
pub type TextStream = BoxStream<'static, Result<String, LlmError>>;

#[async_trait]
pub trait TextGenerationClient: Send + Sync {
    /// Stream text chunks as the provider produces them.
    async fn generate_text_stream(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<TextStream, LlmError>;

    /// Stream text chunks with a prior conversation transcript included.
    /// `history` is the turns the client has already shown (oldest first);
    /// `user_prompt` is the new user turn the model should reply to.
    /// Default impl ignores history and forwards to `generate_text_stream`,
    /// so providers that haven't opted in keep working unchanged.
    async fn generate_text_stream_with_history(
        &self,
        system_prompt: &str,
        _history: &[Turn],
        user_prompt: &str,
    ) -> Result<TextStream, LlmError> {
        self.generate_text_stream(system_prompt, user_prompt).await
    }

    /// Collect the streamed output into a single `String`. Default
    /// implementation consumes `generate_text_stream` to completion.
    async fn generate_text(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<String, LlmError> {
        let mut stream = self
            .generate_text_stream(system_prompt, user_prompt)
            .await?;
        let mut buf = String::new();
        while let Some(chunk) = stream.next().await {
            buf.push_str(&chunk?);
        }
        Ok(buf)
    }
}
