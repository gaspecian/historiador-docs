//! LLM provider probe: a minimal authenticated call to confirm an
//! API key is valid. Used inside the setup wizard so a workspace is
//! never born with an unusable key.
//!
//! The trait exists so the e2e integration test can swap in a stub
//! (`StubProbe`) that never touches the network. Production always
//! uses [`HttpLlmProbe`].

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, utoipa::ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum LlmProvider {
    OpenAi,
    Anthropic,
    /// Local Llama (and friends) served via Ollama. For this provider
    /// the setup wizard's `llm_api_key` field carries the Ollama base
    /// URL (e.g. `http://localhost:11434`) rather than an API key —
    /// Ollama is typically unauthenticated.
    Ollama,
    /// No-op provider for local dev. The probe always succeeds and
    /// the `llm_api_key` value is ignored. Lets you complete setup
    /// without any LLM service running.
    Test,
}

impl LlmProvider {
    pub fn as_db_str(&self) -> &'static str {
        match self {
            LlmProvider::OpenAi => "openai",
            LlmProvider::Anthropic => "anthropic",
            LlmProvider::Ollama => "ollama",
            LlmProvider::Test => "test",
        }
    }
}

#[async_trait]
pub trait LlmProbe: Send + Sync + 'static {
    /// Make a minimal authenticated call against the provider.
    /// `base_url` is honored for `OpenAi` only (Anthropic + Ollama
    /// have provider-specific URL handling already; `Test` ignores it).
    async fn probe(
        &self,
        provider: LlmProvider,
        api_key: &str,
        base_url: Option<&str>,
    ) -> anyhow::Result<()>;
}

/// Real probe — issues one HTTP request per call. No retries; a
/// flaky network at setup time is better surfaced immediately.
pub struct HttpLlmProbe {
    client: reqwest::Client,
}

impl Default for HttpLlmProbe {
    fn default() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("failed to build reqwest client"),
        }
    }
}

#[async_trait]
impl LlmProbe for HttpLlmProbe {
    async fn probe(
        &self,
        provider: LlmProvider,
        api_key: &str,
        base_url: Option<&str>,
    ) -> anyhow::Result<()> {
        match provider {
            LlmProvider::OpenAi => {
                // Validate URL + auth shape via the OpenAI catalog endpoint.
                // No model name is hardcoded — the caller's chosen model is
                // validated implicitly when the chunk pipeline / chat API
                // first hits it.
                let base = base_url
                    .map(|u| u.trim_end_matches('/').to_string())
                    .unwrap_or_else(|| "https://api.openai.com/v1".to_string());
                let url = format!("{base}/models");
                let mut req = self.client.get(&url);
                if !api_key.is_empty() {
                    req = req.bearer_auth(api_key);
                }
                let resp = req
                    .send()
                    .await
                    .map_err(|e| anyhow::anyhow!("openai unreachable at {base}: {e}"))?;
                if !resp.status().is_success() {
                    anyhow::bail!(
                        "openai rejected (status {}) at {base}",
                        resp.status().as_u16()
                    );
                }
                Ok(())
            }
            LlmProvider::Anthropic => {
                // Minimal /v1/messages call with max_tokens = 1.
                let body = serde_json::json!({
                    "model": "claude-haiku-4-5-20251001",
                    "max_tokens": 1,
                    "messages": [{"role": "user", "content": "ping"}],
                });
                let resp = self
                    .client
                    .post("https://api.anthropic.com/v1/messages")
                    .header("x-api-key", api_key)
                    .header("anthropic-version", "2023-06-01")
                    .json(&body)
                    .send()
                    .await?;
                if !resp.status().is_success() {
                    anyhow::bail!(
                        "anthropic rejected the api key (status {})",
                        resp.status().as_u16()
                    );
                }
                Ok(())
            }
            LlmProvider::Ollama => {
                // For Ollama the "api_key" field carries the base URL
                // of a reachable Ollama server. We probe `GET /api/tags`
                // — unauthenticated, cheap, and fails fast if the
                // server is unreachable or not actually Ollama.
                let base = api_key.trim().trim_end_matches('/');
                if base.is_empty() {
                    anyhow::bail!("ollama base URL is empty");
                }
                if !(base.starts_with("http://") || base.starts_with("https://")) {
                    anyhow::bail!(
                        "ollama base URL must start with http:// or https:// (got {base:?})"
                    );
                }
                let url = format!("{base}/api/tags");
                let resp = self.client.get(&url).send().await?;
                if !resp.status().is_success() {
                    anyhow::bail!(
                        "ollama unreachable at {base} (status {})",
                        resp.status().as_u16()
                    );
                }
                Ok(())
            }
            LlmProvider::Test => {
                tracing::info!("test provider — probe skipped");
                Ok(())
            }
        }
    }
}

/// Test stub — always succeeds. Used by `sprint2_e2e.rs`.
pub struct StubProbe;

#[async_trait]
impl LlmProbe for StubProbe {
    async fn probe(
        &self,
        _provider: LlmProvider,
        _api_key: &str,
        _base_url: Option<&str>,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn models_response() -> serde_json::Value {
        serde_json::json!({
            "object": "list",
            "data": [
                {"id": "gpt-4o-mini", "object": "model", "created": 0, "owned_by": "openai"},
                {"id": "text-embedding-3-small", "object": "model", "created": 0, "owned_by": "openai"}
            ]
        })
    }

    #[tokio::test]
    async fn openai_probe_hits_supplied_base_url_with_bearer() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(models_response()))
            .mount(&server)
            .await;

        let probe = HttpLlmProbe::default();
        probe
            .probe(LlmProvider::OpenAi, "secret-key", Some(&server.uri()))
            .await
            .expect("probe ok");

        let recv = server.received_requests().await.unwrap();
        assert_eq!(recv.len(), 1);
        assert_eq!(recv[0].method, wiremock::http::Method::GET);
        assert_eq!(
            recv[0].headers.get("authorization").unwrap(),
            "Bearer secret-key"
        );
    }

    #[tokio::test]
    async fn openai_probe_no_auth_when_key_empty_and_url_set() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(models_response()))
            .mount(&server)
            .await;

        let probe = HttpLlmProbe::default();
        probe
            .probe(LlmProvider::OpenAi, "", Some(&server.uri()))
            .await
            .expect("probe ok");

        let recv = server.received_requests().await.unwrap();
        assert!(recv[0].headers.get("authorization").is_none());
    }

    #[tokio::test]
    async fn openai_probe_surfaces_4xx_as_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/models"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let probe = HttpLlmProbe::default();
        let err = probe
            .probe(LlmProvider::OpenAi, "wrong-key", Some(&server.uri()))
            .await
            .expect_err("expected error");
        assert!(format!("{err}").to_lowercase().contains("openai rejected"));
    }
}
