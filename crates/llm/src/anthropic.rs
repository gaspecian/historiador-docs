//! Anthropic provider implementation for text generation.
//!
//! Uses raw `reqwest` because there is no official Anthropic Rust SDK.
//! Anthropic does not offer an embedding API, so this module only
//! provides text generation. When the workspace provider is Anthropic,
//! embeddings fall back to OpenAI or the stub.

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::LlmError;
use crate::text_generation::TextGenerationClient;

/// Anthropic text generation client using the Messages API.
pub struct AnthropicTextGenerationClient {
    http: Client,
    api_key: String,
    model: String,
}

impl AnthropicTextGenerationClient {
    pub fn new(api_key: &str) -> Self {
        Self::with_model(api_key, "claude-haiku-4-5-20251001")
    }

    pub fn with_model(api_key: &str, model: &str) -> Self {
        Self {
            http: Client::new(),
            api_key: api_key.to_string(),
            model: model.to_string(),
        }
    }
}

#[derive(Serialize)]
struct MessagesRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<Message>,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct MessagesResponse {
    content: Vec<ContentBlock>,
}

#[derive(Deserialize)]
struct ContentBlock {
    text: Option<String>,
}

#[async_trait]
impl TextGenerationClient for AnthropicTextGenerationClient {
    async fn generate_text(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<String, LlmError> {
        let body = MessagesRequest {
            model: self.model.clone(),
            max_tokens: 4096,
            system: system_prompt.to_string(),
            messages: vec![Message {
                role: "user".to_string(),
                content: user_prompt.to_string(),
            }],
        };

        let resp = self
            .http
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(LlmError::Api {
                message: format!("Anthropic API {status}: {text}"),
            });
        }

        let parsed: MessagesResponse = resp.json().await.map_err(|e| LlmError::Api {
            message: format!("failed to parse Anthropic response: {e}"),
        })?;

        let text = parsed
            .content
            .first()
            .and_then(|b| b.text.clone())
            .unwrap_or_default();

        Ok(text)
    }
}
