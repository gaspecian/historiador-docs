//! OpenAI provider implementations for embeddings and text generation.

use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestUserMessageArgs, CreateChatCompletionRequestArgs,
        CreateEmbeddingRequestArgs,
    },
    Client,
};
use async_trait::async_trait;
use futures::StreamExt;

use crate::text_generation::{TextGenerationClient, TextStream};
use crate::{Embedding, EmbeddingClient, LlmError};

/// OpenAI embedding client using `text-embedding-3-small` (1536 dims).
pub struct OpenAiEmbeddingClient {
    client: Client<OpenAIConfig>,
    model: String,
    dim: usize,
}

impl OpenAiEmbeddingClient {
    pub fn new(api_key: &str) -> Self {
        Self::with_model(api_key, "text-embedding-3-small", 1536)
    }

    pub fn with_model(api_key: &str, model: &str, dim: usize) -> Self {
        let config = OpenAIConfig::new().with_api_key(api_key);
        Self {
            client: Client::with_config(config),
            model: model.to_string(),
            dim,
        }
    }
}

#[async_trait]
impl EmbeddingClient for OpenAiEmbeddingClient {
    async fn embed(&self, texts: &[String]) -> Result<Vec<Embedding>, LlmError> {
        if texts.is_empty() {
            return Ok(vec![]);
        }

        let request = CreateEmbeddingRequestArgs::default()
            .model(&self.model)
            .input(texts.to_vec())
            .build()
            .map_err(|e| LlmError::Api {
                message: format!("failed to build embedding request: {e}"),
            })?;

        let response = self
            .client
            .embeddings()
            .create(request)
            .await
            .map_err(|e| LlmError::Api {
                message: format!("OpenAI embedding API error: {e}"),
            })?;

        let embeddings = response
            .data
            .into_iter()
            .map(|d| Embedding {
                vector: d.embedding,
            })
            .collect();

        Ok(embeddings)
    }

    fn dimension(&self) -> usize {
        self.dim
    }
}

/// OpenAI text generation client using chat completions.
pub struct OpenAiTextGenerationClient {
    client: Client<OpenAIConfig>,
    model: String,
}

impl OpenAiTextGenerationClient {
    pub fn new(api_key: &str) -> Self {
        Self::with_model(api_key, "gpt-4o-mini")
    }

    pub fn with_model(api_key: &str, model: &str) -> Self {
        let config = OpenAIConfig::new().with_api_key(api_key);
        Self {
            client: Client::with_config(config),
            model: model.to_string(),
        }
    }
}

#[async_trait]
impl TextGenerationClient for OpenAiTextGenerationClient {
    async fn generate_text_stream(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<TextStream, LlmError> {
        let messages: Vec<ChatCompletionRequestMessage> = vec![
            ChatCompletionRequestSystemMessageArgs::default()
                .content(system_prompt)
                .build()
                .map_err(|e| LlmError::Api {
                    message: format!("failed to build system message: {e}"),
                })?
                .into(),
            ChatCompletionRequestUserMessageArgs::default()
                .content(user_prompt)
                .build()
                .map_err(|e| LlmError::Api {
                    message: format!("failed to build user message: {e}"),
                })?
                .into(),
        ];

        let request = CreateChatCompletionRequestArgs::default()
            .model(&self.model)
            .messages(messages)
            .build()
            .map_err(|e| LlmError::Api {
                message: format!("failed to build chat request: {e}"),
            })?;

        let upstream = self
            .client
            .chat()
            .create_stream(request)
            .await
            .map_err(|e| LlmError::Api {
                message: format!("OpenAI stream init error: {e}"),
            })?;

        let mapped = upstream.map(|item| match item {
            Ok(resp) => Ok(resp
                .choices
                .first()
                .and_then(|c| c.delta.content.clone())
                .unwrap_or_default()),
            Err(e) => Err(LlmError::Api {
                message: format!("OpenAI stream error: {e}"),
            }),
        });

        Ok(Box::pin(mapped))
    }
}

/// Tool-calling placeholder for OpenAI.
///
/// OpenAI's Chat Completions API carries tool calls as a per-choice
/// `tool_calls` array with `function.name` and stringified
/// `function.arguments`. Streaming deltas accumulate on the same
/// index; a call is complete when the choice's `finish_reason` is
/// `tool_calls` (or the final chunk arrives for that index).
///
/// Integration plan mirrors the Anthropic module:
/// 1. Map `ToolSpec` → `tools: [{type: "function", function: {name,
///    description, parameters: <input_schema>}}]`.
/// 2. Track per-index accumulators for `function.arguments`; emit a
///    `ToolStreamItem::ToolCall` once the tool call completes.
/// 3. Text deltas flow through as `ToolStreamItem::Text`.
///
/// Returns `NotImplemented` until that wiring lands.
#[async_trait]
impl crate::tool_calling::ToolCallingClient for OpenAiTextGenerationClient {
    async fn generate_with_tools(
        &self,
        _system_prompt: &str,
        _messages: &[crate::tool_calling::Turn],
        _tools: &[historiador_tools::ToolSpec],
    ) -> Result<crate::tool_calling::ToolStream, LlmError> {
        Err(LlmError::NotImplemented)
    }
}
