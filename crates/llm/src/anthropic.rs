//! Anthropic provider implementation for text generation.
//!
//! Uses raw `reqwest` because there is no official Anthropic Rust SDK.
//! Anthropic does not offer an embedding API, so this module only
//! provides text generation. When the workspace provider is Anthropic,
//! embeddings fall back to OpenAI or the stub.

use async_stream::try_stream;
use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::io;
use tokio::io::AsyncBufReadExt;
use tokio_util::io::StreamReader;

use crate::text_generation::{TextGenerationClient, TextStream};
use crate::LlmError;

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
    stream: bool,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum StreamEvent {
    #[serde(rename = "content_block_delta")]
    ContentBlockDelta { delta: DeltaBlock },
    #[serde(rename = "message_stop")]
    MessageStop,
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum DeltaBlock {
    #[serde(rename = "text_delta")]
    TextDelta { text: String },
    #[serde(other)]
    Other,
}

#[async_trait]
impl TextGenerationClient for AnthropicTextGenerationClient {
    async fn generate_text_stream(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<TextStream, LlmError> {
        let body = MessagesRequest {
            model: self.model.clone(),
            max_tokens: 4096,
            system: system_prompt.to_string(),
            messages: vec![Message {
                role: "user".to_string(),
                content: user_prompt.to_string(),
            }],
            stream: true,
        };

        let resp = self
            .http
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .header("accept", "text/event-stream")
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

        let byte_stream = resp.bytes_stream().map(|r| r.map_err(io::Error::other));
        let reader = StreamReader::new(byte_stream);
        let mut lines = reader.lines();

        let stream = try_stream! {
            while let Some(line) = lines
                .next_line()
                .await
                .map_err(|e| LlmError::Api { message: format!("anthropic sse read: {e}") })?
            {
                let Some(data) = line.strip_prefix("data: ") else {
                    continue;
                };
                let event: StreamEvent = match serde_json::from_str(data) {
                    Ok(ev) => ev,
                    Err(_) => continue,
                };
                match event {
                    StreamEvent::ContentBlockDelta { delta: DeltaBlock::TextDelta { text } } => {
                        yield text;
                    }
                    StreamEvent::MessageStop => break,
                    _ => {}
                }
            }
        };

        Ok(Box::pin(stream))
    }
}
