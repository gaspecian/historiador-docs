//! OpenAI provider implementation for text generation.

use async_openai::{
    types::{
        ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestMessage,
        ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestUserMessageArgs,
        CreateChatCompletionRequestArgs,
    },
    Client,
};
use async_trait::async_trait;
use futures::StreamExt;

use crate::openai_compat::OpenAiCompatConfig;
use crate::text_generation::{TextGenerationClient, TextStream};
use crate::tool_calling::Turn;
use crate::LlmError;

/// Builder shape for [`OpenAiTextGenerationClient::from_config`]. Set
/// `base_url = None` to use the canonical OpenAI endpoint and
/// `api_key = None` to omit the `Authorization` header.
pub struct OpenAiGenerationConfig<'a> {
    pub api_key: Option<&'a str>,
    pub base_url: Option<&'a str>,
    pub model: &'a str,
}

pub struct OpenAiTextGenerationClient {
    client: Client<OpenAiCompatConfig>,
    model: String,
}

impl OpenAiTextGenerationClient {
    pub fn new(api_key: &str) -> Self {
        Self::with_model(api_key, "gpt-4o-mini")
    }

    pub fn with_model(api_key: &str, model: &str) -> Self {
        Self::from_config(OpenAiGenerationConfig {
            api_key: Some(api_key),
            base_url: None,
            model,
        })
    }

    pub fn from_config(cfg: OpenAiGenerationConfig<'_>) -> Self {
        let config = OpenAiCompatConfig::new(cfg.base_url, cfg.api_key);
        Self {
            client: Client::with_config(config),
            model: cfg.model.to_string(),
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
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn chat_with_custom_base_url_and_no_auth() {
        use crate::text_generation::TextGenerationClient;

        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "x",
                "object": "chat.completion",
                "created": 0,
                "model": "any",
                "choices": [{
                    "index": 0,
                    "message": {"role": "assistant", "content": "ok"},
                    "finish_reason": "stop"
                }],
                "usage": {"prompt_tokens": 1, "completion_tokens": 1, "total_tokens": 2}
            })))
            .mount(&server)
            .await;

        let client = OpenAiTextGenerationClient::from_config(OpenAiGenerationConfig {
            api_key: None,
            base_url: Some(&server.uri()),
            model: "any",
        });
        let mut stream = client
            .generate_text_stream("be brief", "hi")
            .await
            .expect("stream ok");
        // The chat stream is lazy — polling is what fires the HTTP
        // request. Wiremock returns JSON (not SSE), so the body parse
        // will error, but we only care that the request reached the
        // server with the right URL + no Authorization header.
        let _ = stream.next().await;

        let received = server.received_requests().await.unwrap();
        assert!(
            received.iter().any(|r| r.url.path() == "/chat/completions"),
            "expected POST /chat/completions"
        );
        assert!(
            received[0].headers.get("authorization").is_none(),
            "no Authorization header should be sent"
        );
    }
}
