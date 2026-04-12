//! `TextGenerationClient` trait for LLM text completion.

use async_trait::async_trait;

use crate::LlmError;

/// Trait for generating text from a prompt. Separate from
/// [`crate::EmbeddingClient`] because most callers need only one
/// capability: the MCP server needs embeddings, the editor needs
/// text generation, and the publish pipeline needs embeddings.
#[async_trait]
pub trait TextGenerationClient: Send + Sync {
    /// Generate text given a system prompt and a user prompt.
    ///
    /// Implementations map these to the provider's native format
    /// (e.g. OpenAI's `messages` array, Anthropic's `system` +
    /// `messages`).
    async fn generate_text(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<String, LlmError>;
}
