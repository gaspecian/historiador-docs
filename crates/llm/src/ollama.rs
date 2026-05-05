//! Ollama provider implementation.
//!
//! Ollama is a local LLM runtime. Text generation uses `POST /api/generate`
//! with `stream: true` returning newline-delimited JSON.

use async_stream::try_stream;
use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::io::AsyncBufReadExt;
use tokio_util::io::StreamReader;

use crate::text_generation::{TextGenerationClient, TextStream};
use crate::tool_calling::Turn;
use crate::LlmError;

fn default_http_client() -> Client {
    Client::builder()
        // Local model first-token latency can be large for big prompts.
        .timeout(Duration::from_secs(600))
        .build()
        .expect("reqwest client builds")
}

fn normalize_base_url(raw: &str) -> String {
    raw.trim_end_matches('/').to_string()
}

// ---------- text generation ----------

pub struct OllamaTextClient {
    http: Client,
    base_url: String,
    model: String,
}

impl OllamaTextClient {
    pub fn new(base_url: &str, model: &str) -> Self {
        Self {
            http: default_http_client(),
            base_url: normalize_base_url(base_url),
            model: model.to_string(),
        }
    }
}

#[derive(Serialize)]
struct GenerateRequest<'a> {
    model: &'a str,
    system: &'a str,
    prompt: &'a str,
    stream: bool,
}

#[derive(Deserialize)]
struct GenerateChunk {
    #[serde(default)]
    response: String,
    #[serde(default)]
    done: bool,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
    stream: bool,
}

#[derive(Deserialize)]
struct ChatChunkMessage {
    #[serde(default)]
    content: String,
}

#[derive(Deserialize)]
struct ChatChunk {
    #[serde(default)]
    message: Option<ChatChunkMessage>,
    #[serde(default)]
    done: bool,
    #[serde(default)]
    error: Option<String>,
}

#[async_trait]
impl TextGenerationClient for OllamaTextClient {
    async fn generate_text_stream(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<TextStream, LlmError> {
        let url = format!("{}/api/generate", self.base_url);
        let body = GenerateRequest {
            model: &self.model,
            system: system_prompt,
            prompt: user_prompt,
            stream: true,
        };

        let resp = self.http.post(&url).json(&body).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(LlmError::Api {
                message: format!("Ollama API {status}: {text}"),
            });
        }

        let byte_stream = resp
            .bytes_stream()
            .map(|r| r.map_err(std::io::Error::other));
        let reader = StreamReader::new(byte_stream);
        let mut lines = reader.lines();

        let stream = try_stream! {
            while let Some(line) = lines
                .next_line()
                .await
                .map_err(|e| LlmError::Api { message: format!("ollama stream read: {e}") })?
            {
                if line.is_empty() {
                    continue;
                }
                let chunk: GenerateChunk = serde_json::from_str(&line)
                    .map_err(|e| LlmError::Api {
                        message: format!("ollama ndjson parse: {e}"),
                    })?;
                if let Some(err) = chunk.error {
                    Err(LlmError::Api { message: format!("ollama: {err}") })?;
                }
                if !chunk.response.is_empty() {
                    yield chunk.response;
                }
                if chunk.done {
                    break;
                }
            }
        };

        Ok(Box::pin(stream))
    }

    async fn generate_text_stream_with_history(
        &self,
        system_prompt: &str,
        history: &[Turn],
        user_prompt: &str,
    ) -> Result<TextStream, LlmError> {
        // History path switches to /api/chat, which takes a messages
        // array and preserves role separation for the model. The
        // no-history path stays on /api/generate above so existing
        // single-turn callers keep their exact semantics.
        let url = format!("{}/api/chat", self.base_url);
        let mut messages: Vec<ChatMessage> = Vec::with_capacity(2 + history.len());
        messages.push(ChatMessage {
            role: "system",
            content: system_prompt,
        });
        for turn in history {
            if turn.role == "user" || turn.role == "assistant" {
                messages.push(ChatMessage {
                    role: turn.role.as_str(),
                    content: turn.content.as_str(),
                });
            }
        }
        messages.push(ChatMessage {
            role: "user",
            content: user_prompt,
        });

        let body = ChatRequest {
            model: &self.model,
            messages,
            stream: true,
        };

        let resp = self.http.post(&url).json(&body).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(LlmError::Api {
                message: format!("Ollama API {status}: {text}"),
            });
        }

        let byte_stream = resp
            .bytes_stream()
            .map(|r| r.map_err(std::io::Error::other));
        let reader = StreamReader::new(byte_stream);
        let mut lines = reader.lines();

        let stream = try_stream! {
            while let Some(line) = lines
                .next_line()
                .await
                .map_err(|e| LlmError::Api { message: format!("ollama stream read: {e}") })?
            {
                if line.is_empty() {
                    continue;
                }
                let chunk: ChatChunk = serde_json::from_str(&line)
                    .map_err(|e| LlmError::Api {
                        message: format!("ollama ndjson parse: {e}"),
                    })?;
                if let Some(err) = chunk.error {
                    Err(LlmError::Api { message: format!("ollama: {err}") })?;
                }
                if let Some(msg) = chunk.message {
                    if !msg.content.is_empty() {
                        yield msg.content;
                    }
                }
                if chunk.done {
                    break;
                }
            }
        };

        Ok(Box::pin(stream))
    }
}

/// Tool-calling placeholder for Ollama.
///
/// Ollama's `/api/chat` endpoint supports a `tools` parameter and
/// emits `tool_calls` inside message objects when the underlying
/// model supports it (e.g., Llama 3.1+, Qwen 2.5). For models
/// without native tool support, the provider-agnostic fallback is
/// to instruct the model via the system prompt to emit a specific
/// JSON schema and parse the response out.
///
/// Returns `NotImplemented` until the adapter is wired; the
/// dispatcher falls back to text-only generation in the meantime.
#[async_trait]
impl crate::tool_calling::ToolCallingClient for OllamaTextClient {
    async fn generate_with_tools(
        &self,
        _system_prompt: &str,
        _messages: &[crate::tool_calling::Turn],
        _tools: &[historiador_tools::ToolSpec],
    ) -> Result<crate::tool_calling::ToolStream, LlmError> {
        Err(LlmError::NotImplemented)
    }
}

// ---------- misc ----------

#[derive(Debug, Deserialize)]
pub struct OllamaTag {
    pub name: String,
    #[serde(default)]
    pub size: u64,
}

#[derive(Debug, Deserialize)]
struct TagsResponse {
    models: Vec<OllamaTag>,
}

/// List the locally available models on an Ollama server. Used by the
/// setup wizard and admin panel to populate model dropdowns.
pub async fn list_models(base_url: &str) -> Result<Vec<OllamaTag>, LlmError> {
    let http = default_http_client();
    let url = format!("{}/api/tags", normalize_base_url(base_url));
    let resp = http.get(&url).send().await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(LlmError::Api {
            message: format!("Ollama /api/tags {status}: {body}"),
        });
    }
    let parsed: TagsResponse = resp.json().await.map_err(|e| LlmError::Api {
        message: format!("ollama tags parse: {e}"),
    })?;
    Ok(parsed.models)
}

// ---------- optional integration tests (gated) ----------

#[cfg(all(test, feature = "ollama-tests"))]
mod integration_tests {
    use super::*;
    use futures::StreamExt;

    fn base_url() -> Option<String> {
        std::env::var("OLLAMA_BASE_URL").ok()
    }

    fn gen_model() -> String {
        std::env::var("OLLAMA_TEST_GEN_MODEL").unwrap_or_else(|_| "llama3.2:1b".into())
    }

    #[tokio::test]
    async fn text_stream_round_trip() {
        let Some(url) = base_url() else {
            eprintln!("skipping: OLLAMA_BASE_URL unset");
            return;
        };
        let client = OllamaTextClient::new(&url, &gen_model());
        let mut stream = client
            .generate_text_stream("Reply in one short sentence.", "Say hi.")
            .await
            .expect("stream init");
        let mut collected = String::new();
        while let Some(chunk) = stream.next().await {
            collected.push_str(&chunk.expect("chunk"));
        }
        assert!(!collected.trim().is_empty(), "expected non-empty output");
    }

    #[tokio::test]
    async fn list_models_returns_nonempty() {
        let Some(url) = base_url() else {
            eprintln!("skipping: OLLAMA_BASE_URL unset");
            return;
        };
        let models = list_models(&url).await.expect("tags");
        assert!(!models.is_empty(), "expected at least one model pulled");
    }
}
