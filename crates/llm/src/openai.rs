//! OpenAI provider implementations for embeddings and text generation.

use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestMessage,
        ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
        CreateChatCompletionRequestArgs, CreateEmbeddingRequestArgs,
    },
    Client,
};
use async_trait::async_trait;
use futures::StreamExt;

use crate::openai_compat::OpenAiCompatConfig;
use crate::text_generation::{TextGenerationClient, TextStream};
use crate::tool_calling::Turn;
use crate::{Embedding, EmbeddingClient, LlmError};

pub struct OpenAiEmbeddingConfig<'a> {
    pub api_key: Option<&'a str>,
    pub base_url: Option<&'a str>,
    pub model: &'a str,
    pub dim: usize,
}

/// OpenAI-compatible embedding client. Defaults to
/// `text-embedding-3-small` (1536 dims) on the canonical OpenAI
/// endpoint when constructed with `new`.
pub struct OpenAiEmbeddingClient {
    client: Client<OpenAiCompatConfig>,
    model: String,
    dim: usize,
}

impl OpenAiEmbeddingClient {
    pub fn new(api_key: &str) -> Self {
        Self::with_model(api_key, "text-embedding-3-small", 1536)
    }

    pub fn with_model(api_key: &str, model: &str, dim: usize) -> Self {
        Self::from_config(OpenAiEmbeddingConfig {
            api_key: Some(api_key),
            base_url: None,
            model,
            dim,
        })
    }

    pub fn from_config(cfg: OpenAiEmbeddingConfig<'_>) -> Self {
        let config = OpenAiCompatConfig::new(cfg.base_url, cfg.api_key);
        Self {
            client: Client::with_config(config),
            model: cfg.model.to_string(),
            dim: cfg.dim,
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
        self.generate_text_stream_with_history(system_prompt, &[], user_prompt)
            .await
    }

    async fn generate_text_stream_with_history(
        &self,
        system_prompt: &str,
        history: &[Turn],
        user_prompt: &str,
    ) -> Result<TextStream, LlmError> {
        let mut messages: Vec<ChatCompletionRequestMessage> =
            Vec::with_capacity(1 + history.len() + 1);
        messages.push(
            ChatCompletionRequestSystemMessageArgs::default()
                .content(system_prompt)
                .build()
                .map_err(|e| LlmError::Api {
                    message: format!("failed to build system message: {e}"),
                })?
                .into(),
        );
        for turn in history {
            match turn.role.as_str() {
                "user" => messages.push(
                    ChatCompletionRequestUserMessageArgs::default()
                        .content(turn.content.clone())
                        .build()
                        .map_err(|e| LlmError::Api {
                            message: format!("failed to build user history message: {e}"),
                        })?
                        .into(),
                ),
                "assistant" => messages.push(
                    ChatCompletionRequestAssistantMessageArgs::default()
                        .content(turn.content.clone())
                        .build()
                        .map_err(|e| LlmError::Api {
                            message: format!("failed to build assistant history message: {e}"),
                        })?
                        .into(),
                ),
                _ => {
                    // Silently drop unrecognised roles (e.g. "system" sneaking
                    // into history): OpenAI only accepts user/assistant after
                    // the initial system turn we already prepended.
                }
            }
        }
        messages.push(
            ChatCompletionRequestUserMessageArgs::default()
                .content(user_prompt)
                .build()
                .map_err(|e| LlmError::Api {
                    message: format!("failed to build user message: {e}"),
                })?
                .into(),
        );

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EmbeddingClient;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn embed_with_custom_base_url_and_bearer() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/embeddings"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "object": "list",
                "data": [{"object": "embedding", "index": 0, "embedding": [0.1, 0.2, 0.3]}],
                "model": "text-embedding-3-small",
                "usage": {"prompt_tokens": 1, "total_tokens": 1}
            })))
            .mount(&server)
            .await;

        let client = OpenAiEmbeddingClient::from_config(OpenAiEmbeddingConfig {
            api_key: Some("my-key"),
            base_url: Some(&server.uri()),
            model: "text-embedding-3-small",
            dim: 3,
        });
        let out = client.embed(&["hello".into()]).await.expect("embed ok");
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].vector.len(), 3);

        let recv = server.received_requests().await.unwrap();
        assert_eq!(recv.len(), 1);
        assert_eq!(
            recv[0].headers.get("authorization").unwrap(),
            "Bearer my-key"
        );
    }

    #[tokio::test]
    async fn embed_without_authorization_header() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/embeddings"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "object": "list",
                "data": [{"object": "embedding", "index": 0, "embedding": [0.0]}],
                "model": "any",
                "usage": {"prompt_tokens": 1, "total_tokens": 1}
            })))
            .mount(&server)
            .await;

        let client = OpenAiEmbeddingClient::from_config(OpenAiEmbeddingConfig {
            api_key: None,
            base_url: Some(&server.uri()),
            model: "any",
            dim: 1,
        });
        client.embed(&["hi".into()]).await.expect("embed ok");

        let received = server.received_requests().await.unwrap();
        assert_eq!(received.len(), 1);
        assert!(
            received[0].headers.get("authorization").is_none(),
            "no Authorization header should be sent"
        );
    }
}
