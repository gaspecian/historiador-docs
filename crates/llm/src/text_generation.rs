//! `TextGenerationClient` trait for LLM text completion.
//!
//! The primitive is streaming: implementations yield text chunks as the
//! upstream provider produces them. A default `generate_text` collects
//! the stream into a single `String` for callers that do not care about
//! incremental output.

use async_trait::async_trait;
use futures::stream::BoxStream;
use futures::StreamExt;

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
