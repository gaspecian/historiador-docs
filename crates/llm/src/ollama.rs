//! Ollama provider implementation.
//!
//! Ollama is a local LLM runtime. Text generation uses `POST /api/generate`
//! with `stream: true` returning newline-delimited JSON; embeddings use
//! `POST /api/embeddings` which returns a single vector per request.
//!
//! The embedding dimension depends on the model (e.g. `nomic-embed-text`
//! returns 768, `mxbai-embed-large` returns 1024). We discover it on the
//! first successful call and cache it.

use async_stream::try_stream;
use async_trait::async_trait;
use futures::{stream::FuturesOrdered, StreamExt, TryStreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncBufReadExt;
use tokio::sync::OnceCell;
use tokio_util::io::StreamReader;

use crate::text_generation::{TextGenerationClient, TextStream};
use crate::{Embedding, EmbeddingClient, LlmError};

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
}

// ---------- embeddings ----------

pub struct OllamaEmbeddingClient {
    http: Client,
    base_url: String,
    model: String,
    dim: Arc<OnceCell<usize>>,
}

impl OllamaEmbeddingClient {
    pub fn new(base_url: &str, model: &str) -> Self {
        Self {
            http: default_http_client(),
            base_url: normalize_base_url(base_url),
            model: model.to_string(),
            dim: Arc::new(OnceCell::new()),
        }
    }

    /// Preflight: calls `/api/embeddings` once with a tiny prompt so
    /// `dimension()` returns the real model dimension immediately.
    pub async fn preflight(&self) -> Result<usize, LlmError> {
        let emb = self.embed_one("historiador").await?;
        let d = emb.vector.len();
        let _ = self.dim.set(d);
        Ok(d)
    }

    async fn embed_one(&self, text: &str) -> Result<Embedding, LlmError> {
        #[derive(Serialize)]
        struct Req<'a> {
            model: &'a str,
            prompt: &'a str,
        }
        #[derive(Deserialize)]
        struct Resp {
            embedding: Vec<f32>,
            #[serde(default)]
            error: Option<String>,
        }

        let url = format!("{}/api/embeddings", self.base_url);
        let resp = self
            .http
            .post(&url)
            .json(&Req {
                model: &self.model,
                prompt: text,
            })
            .send()
            .await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(LlmError::Api {
                message: format!("Ollama embeddings {status}: {body}"),
            });
        }
        let parsed: Resp = resp.json().await.map_err(|e| LlmError::Api {
            message: format!("ollama embeddings parse: {e}"),
        })?;
        if let Some(err) = parsed.error {
            return Err(LlmError::Api {
                message: format!("ollama: {err}"),
            });
        }
        Ok(Embedding {
            vector: parsed.embedding,
        })
    }
}

#[async_trait]
impl EmbeddingClient for OllamaEmbeddingClient {
    async fn embed(&self, texts: &[String]) -> Result<Vec<Embedding>, LlmError> {
        if texts.is_empty() {
            return Ok(vec![]);
        }

        // Ollama's /api/embeddings accepts one prompt per request. Fan out
        // with bounded concurrency to avoid overwhelming a local runtime.
        let mut in_flight = FuturesOrdered::new();
        for text in texts {
            in_flight.push_back(self.embed_one(text));
        }
        let out: Vec<Embedding> = in_flight.try_collect().await?;

        // Cache the dimension from the first response so `dimension()` is
        // accurate and cheap after the first call.
        if let Some(first) = out.first() {
            let _ = self.dim.set(first.vector.len());
        }

        Ok(out)
    }

    fn dimension(&self) -> usize {
        // If `preflight` or `embed` has run, use the cached value. Otherwise
        // fall back to the OpenAI default so schema code that queries
        // dimension before first use does not panic. Callers that care
        // about truth must call `preflight` at boot.
        self.dim.get().copied().unwrap_or(1536)
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

    fn embed_model() -> String {
        std::env::var("OLLAMA_TEST_EMBED_MODEL").unwrap_or_else(|_| "nomic-embed-text".into())
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
    async fn embedding_round_trip_and_dim_cache() {
        let Some(url) = base_url() else {
            eprintln!("skipping: OLLAMA_BASE_URL unset");
            return;
        };
        let client = OllamaEmbeddingClient::new(&url, &embed_model());
        let out = client
            .embed(&["hello".into(), "world".into()])
            .await
            .expect("embed");
        assert_eq!(out.len(), 2);
        assert!(!out[0].vector.is_empty());
        assert_eq!(out[0].vector.len(), out[1].vector.len());
        assert_eq!(client.dimension(), out[0].vector.len());
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
