# Custom Base URL for OpenAI Provider — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Allow workspace admins to point the `openai` LLM provider at any
OpenAI-compatible endpoint (vLLM, LiteLLM, OpenRouter, Azure-via-proxy,
LM Studio, …) and to optionally omit the API key for self-hosted
servers that don't require auth.

**Architecture:** Add a custom `OpenAiCompatConfig` implementing
`async_openai::config::Config`, with conditional `Authorization`
headers. Plumb an `Option<base_url>` through the LLM crate, the
`LlmProbe` trait, the two use cases (`UpdateLlmConfigUseCase`,
`InitializeInstallationUseCase`), the three HTTP DTOs, and the two
frontend forms. The DB column `workspaces.llm_base_url` already
exists from the Ollama work — no migration is needed.

**Tech Stack:** Rust (Axum, sqlx, async-openai 0.27, validator,
utoipa), Next.js 16 / React 19, TypeScript, pnpm, turborepo.

**Spec:** [`artifacts/specs/2026-05-01-openai-custom-base-url-design.md`](../specs/2026-05-01-openai-custom-base-url-design.md)

---

## Pre-flight

- [ ] **Confirm working tree is clean and on branch `feature/accept-url-openai`**

```bash
git status
git rev-parse --abbrev-ref HEAD
```

Expected: clean tree, branch `feature/accept-url-openai`.

- [ ] **Re-read the spec** at
  `artifacts/specs/2026-05-01-openai-custom-base-url-design.md`,
  paying particular attention to §3 (configuration semantics
  matrix), §4 (URL validation rules), and §10 (which is now
  decided: we use a custom `Config` impl).

- [ ] **Re-read [`apps/web/AGENTS.md`](../../apps/web/AGENTS.md)** before any frontend work
  and consult `apps/web/node_modules/next/dist/docs/` for any
  Next.js 16-specific behavior the steps below touch.

---

## Task 1: Add `wiremock` and `secrecy` dependencies

The `wiremock` crate gives us a real HTTP server we can assert
against in unit tests. `secrecy::SecretString` is required because
`async_openai::config::Config::api_key(&self) -> &SecretString` —
we cannot satisfy the trait without it.

**Files:**
- Modify: `Cargo.toml` (workspace dependencies)
- Modify: `crates/llm/Cargo.toml`

- [ ] **Step 1: Add workspace deps**

Add to the `[workspace.dependencies]` table in `Cargo.toml` (top
level):

```toml
secrecy  = "0.10"
wiremock = "0.6"
```

- [ ] **Step 2: Add deps to `crates/llm/Cargo.toml`**

Add `secrecy` to the `[dependencies]` block:

```toml
secrecy.workspace = true
```

Add a new `[dev-dependencies]` block:

```toml
[dev-dependencies]
wiremock.workspace = true
tokio = { workspace = true, features = ["macros", "rt-multi-thread"] }
```

- [ ] **Step 3: Verify the workspace builds**

```bash
cargo build -p historiador_llm
```

Expected: builds clean.

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml crates/llm/Cargo.toml Cargo.lock
git commit -m "chore(llm): add wiremock + secrecy deps for openai compat config"
```

---

## Task 2: Custom `OpenAiCompatConfig` (auth + base URL)

Lives in a new module so its surface stays small.

**Files:**
- Create: `crates/llm/src/openai_compat.rs`
- Modify: `crates/llm/src/lib.rs` (re-export the new module)

- [ ] **Step 1: Write the failing test**

Append to `crates/llm/src/openai_compat.rs` (create file with a
`#[cfg(test)]` block placeholder first if needed):

```rust
//! `Config` impl that supports custom base URLs and optional auth
//! for OpenAI-compatible HTTP endpoints (vLLM, LiteLLM, OpenRouter,
//! Azure-via-proxy, LM Studio, etc.).

use async_openai::config::{Config, OPENAI_API_BASE};
use reqwest::header::{HeaderMap, AUTHORIZATION};
use secrecy::{ExposeSecret, SecretString};

#[derive(Clone, Debug)]
pub struct OpenAiCompatConfig {
    api_base: String,
    api_key: SecretString,
    auth: bool,
}

impl OpenAiCompatConfig {
    /// `base_url = None` → use the canonical `https://api.openai.com/v1`.
    /// `api_key = None` → omit the `Authorization` header entirely
    /// (for unauthenticated self-hosted servers).
    pub fn new(base_url: Option<&str>, api_key: Option<&str>) -> Self {
        let api_base = base_url
            .map(|u| u.trim_end_matches('/').to_string())
            .unwrap_or_else(|| OPENAI_API_BASE.to_string());
        let (key_str, auth) = match api_key {
            Some(k) if !k.is_empty() => (k.to_string(), true),
            _ => (String::new(), false),
        };
        Self {
            api_base,
            api_key: SecretString::from(key_str),
            auth,
        }
    }
}

impl Config for OpenAiCompatConfig {
    fn headers(&self) -> HeaderMap {
        let mut h = HeaderMap::new();
        if self.auth {
            let v = format!("Bearer {}", self.api_key.expose_secret());
            h.insert(AUTHORIZATION, v.parse().expect("valid header value"));
        }
        h
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.api_base, path)
    }

    fn query(&self) -> Vec<(&str, &str)> {
        Vec::new()
    }

    fn api_base(&self) -> &str {
        &self.api_base
    }

    fn api_key(&self) -> &SecretString {
        &self.api_key
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::AUTHORIZATION;

    #[test]
    fn defaults_to_openai_when_base_url_none() {
        let cfg = OpenAiCompatConfig::new(None, Some("sk-test"));
        assert_eq!(cfg.api_base(), "https://api.openai.com/v1");
        assert_eq!(
            cfg.headers().get(AUTHORIZATION).unwrap(),
            "Bearer sk-test"
        );
    }

    #[test]
    fn uses_custom_base_url_with_bearer() {
        let cfg = OpenAiCompatConfig::new(Some("https://litellm.example/v1"), Some("k"));
        assert_eq!(cfg.api_base(), "https://litellm.example/v1");
        assert_eq!(cfg.headers().get(AUTHORIZATION).unwrap(), "Bearer k");
    }

    #[test]
    fn strips_trailing_slash() {
        let cfg = OpenAiCompatConfig::new(Some("https://litellm.example/v1/"), Some("k"));
        assert_eq!(cfg.api_base(), "https://litellm.example/v1");
    }

    #[test]
    fn omits_authorization_when_no_key() {
        let cfg = OpenAiCompatConfig::new(Some("http://localhost:8000/v1"), None);
        assert!(cfg.headers().get(AUTHORIZATION).is_none());
    }

    #[test]
    fn omits_authorization_when_empty_key() {
        let cfg = OpenAiCompatConfig::new(Some("http://localhost:8000/v1"), Some(""));
        assert!(cfg.headers().get(AUTHORIZATION).is_none());
    }
}
```

Add to `crates/llm/src/lib.rs`:

```rust
pub mod openai_compat;
pub use openai_compat::OpenAiCompatConfig;
```

- [ ] **Step 2: Verify tests fail (compile error first; implementation
  is already in the file above so this should compile and pass —
  if so, this step degenerates to a sanity check)**

```bash
cargo test -p historiador_llm openai_compat
```

Expected: 5 tests pass. (The implementation lives in the same file
as the tests; this is a self-contained module.)

- [ ] **Step 3: If any test fails, fix `OpenAiCompatConfig` until all 5 pass.**

- [ ] **Step 4: Commit**

```bash
git add crates/llm/src/openai_compat.rs crates/llm/src/lib.rs
git commit -m "feat(llm): OpenAiCompatConfig with optional base url + auth"
```

---

## Task 3: Refactor `OpenAiEmbeddingClient` to use `OpenAiCompatConfig`

**Files:**
- Modify: `crates/llm/src/openai.rs` (lines 1-79 approximately)

The existing `Client<OpenAIConfig>` field is replaced with
`Client<OpenAiCompatConfig>`. Existing constructors (`new`,
`with_model`) become thin wrappers. New constructor `from_config`
accepts the spec's `OpenAiEmbeddingConfig` shape.

- [ ] **Step 1: Add the failing test (wiremock-driven)**

Add to the existing `#[cfg(test)] mod tests` block of
`crates/llm/src/openai.rs` (create the block if missing):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::EmbeddingClient;
    use wiremock::matchers::{header, header_exists, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn embed_with_custom_base_url_and_bearer() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/embeddings"))
            .and(header("authorization", "Bearer my-key"))
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
    }

    #[tokio::test]
    async fn embed_without_authorization_header() {
        let server = MockServer::start().await;
        // Assert the request does NOT carry an Authorization header.
        // wiremock has no built-in negative matcher, so we inspect
        // the recorded request after the call.
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
```

- [ ] **Step 2: Run the tests — expected FAIL (compile error: `from_config` and `OpenAiEmbeddingConfig` do not exist yet)**

```bash
cargo test -p historiador_llm openai::tests
```

Expected: compile error.

- [ ] **Step 3: Implement `OpenAiEmbeddingConfig` and refactor the client**

Replace the top of `crates/llm/src/openai.rs` (down through the
embedding client `impl` block — roughly lines 1-79) with:

```rust
//! OpenAI provider implementations for embeddings and text generation.

use async_openai::{
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
```

Leave the existing `#[async_trait] impl EmbeddingClient for OpenAiEmbeddingClient`
block exactly as-is — its `self.client.embeddings().create(...)` call works
identically against the new generic `Client<OpenAiCompatConfig>`.

- [ ] **Step 4: Run tests — expect both new tests to pass plus everything else**

```bash
cargo test -p historiador_llm
cargo build --workspace
```

Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add crates/llm/src/openai.rs
git commit -m "feat(llm): OpenAiEmbeddingClient::from_config with optional base url + auth"
```

---

## Task 4: Refactor `OpenAiTextGenerationClient` symmetrically

**Files:**
- Modify: `crates/llm/src/openai.rs` (the text-generation client block, originally lines 82-103)

- [ ] **Step 1: Add the failing test**

Append to the same `#[cfg(test)] mod tests` block in `crates/llm/src/openai.rs`:

```rust
    #[tokio::test]
    async fn chat_with_custom_base_url_and_no_auth() {
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
        let _stream = client
            .generate_text_stream("be brief", "hi")
            .await
            .expect("stream ok");

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
```

- [ ] **Step 2: Run the test — expected FAIL (compile error)**

```bash
cargo test -p historiador_llm openai::tests::chat_with_custom_base_url_and_no_auth
```

Expected: compile error (`OpenAiGenerationConfig` and
`from_config` do not exist on the text-gen client).

- [ ] **Step 3: Refactor `OpenAiTextGenerationClient`**

Locate the block beginning `pub struct OpenAiTextGenerationClient {`
(around line 82) and replace through its `impl` block with:

```rust
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
```

The existing `impl TextGenerationClient for OpenAiTextGenerationClient`
block stays untouched. Same for the
`impl crate::tool_calling::ToolCallingClient`. The generic
`Client<OpenAiCompatConfig>` is type-compatible.

- [ ] **Step 4: Run all crate tests**

```bash
cargo test -p historiador_llm
cargo build --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Expected: all green.

- [ ] **Step 5: Commit**

```bash
git add crates/llm/src/openai.rs
git commit -m "feat(llm): OpenAiTextGenerationClient::from_config with optional base url + auth"
```

---

## Task 5: Extend `LlmProbe` trait with `base_url`

This is a breaking change inside the codebase. Update the trait,
both impls (`HttpLlmProbe`, `StubProbe`), and all 4 callers in one
atomic commit so the tree never goes red.

**Files:**
- Modify: `apps/api/src/infrastructure/llm/probe.rs`
- Modify: `apps/api/src/application/setup/probe_llm.rs`
- Modify: `apps/api/src/application/setup/initialize_installation.rs`
- Modify: `apps/api/src/application/admin/update_llm_config.rs`

- [ ] **Step 1: Extend the trait and update both impls**

In `apps/api/src/infrastructure/llm/probe.rs`, change the trait
declaration (around line 40):

```rust
#[async_trait]
pub trait LlmProbe: Send + Sync + 'static {
    /// Make a minimal authenticated call against the provider.
    /// `base_url` is honored for `OpenAi` only (Anthropic + Ollama
    /// have provider-specific URL handling already; `Test` ignores
    /// it).
    async fn probe(
        &self,
        provider: LlmProvider,
        api_key: &str,
        base_url: Option<&str>,
    ) -> anyhow::Result<()>;
}
```

In the same file, update `impl LlmProbe for HttpLlmProbe`. Replace
the `LlmProvider::OpenAi` arm with logic that uses
`OpenAiEmbeddingClient::from_config` against the supplied URL —
this also implicitly covers the no-auth case:

```rust
async fn probe(
    &self,
    provider: LlmProvider,
    api_key: &str,
    base_url: Option<&str>,
) -> anyhow::Result<()> {
    match provider {
        LlmProvider::OpenAi => {
            // Probe with a 1-token embedding call. This validates
            // both the URL/credentials AND that the embedding
            // endpoint is reachable — which is the only call path
            // we actually need for indexing.
            use historiador_llm::{EmbeddingClient, OpenAiEmbeddingClient, OpenAiEmbeddingConfig};
            let key = if api_key.is_empty() { None } else { Some(api_key) };
            let client = OpenAiEmbeddingClient::from_config(OpenAiEmbeddingConfig {
                api_key: key,
                base_url,
                model: "text-embedding-3-small",
                dim: 1536,
            });
            client
                .embed(&["ping".to_string()])
                .await
                .map_err(|e| anyhow::anyhow!("openai rejected: {e}"))?;
            Ok(())
        }
        LlmProvider::Anthropic => { /* unchanged */ }
        LlmProvider::Ollama    => { /* unchanged */ }
        LlmProvider::Test      => { /* unchanged */ }
    }
}
```

(Keep the bodies of the Anthropic/Ollama/Test arms exactly as they
are today.)

Update `impl LlmProbe for StubProbe` to match the new signature:

```rust
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
```

- [ ] **Step 2: Update `ProbeLlmUseCase`**

In `apps/api/src/application/setup/probe_llm.rs`:

```rust
pub async fn execute(
    &self,
    provider: LlmProvider,
    api_key: &str,
    base_url: Option<&str>,
) -> Result<ProbeLlmResult, ApplicationError> {
    Ok(match self.probe.probe(provider, api_key, base_url).await {
        Ok(()) => ProbeLlmResult { success: true,  message: "connection successful".into() },
        Err(e) => ProbeLlmResult { success: false, message: format!("{e}") },
    })
}
```

- [ ] **Step 3: Update `InitializeInstallationUseCase` and `UpdateLlmConfigUseCase` callers**

In both
`apps/api/src/application/setup/initialize_installation.rs` and
`apps/api/src/application/admin/update_llm_config.rs`, locate the
`self.probe.probe(...)` call and add the new third argument. For
this task we pass `None` everywhere — Task 7/8 will plumb a real
URL through these use cases:

```rust
self.probe.probe(cmd.llm_provider, &cmd.llm_api_key, None).await
```

- [ ] **Step 4: Verify the workspace compiles and existing tests still pass**

```bash
cargo build --workspace
cargo test --workspace
```

Expected: green.

- [ ] **Step 5: Commit**

```bash
git add apps/api/src
git commit -m "refactor(probe): LlmProbe trait gains base_url argument"
```

---

## Task 6: `HttpLlmProbe::probe` honors custom base URL (TDD)

We have the new signature in place but no test that proves the
OpenAI arm actually hits the URL we pass. Add a wiremock-driven
unit test in `probe.rs`.

**Files:**
- Modify: `apps/api/src/infrastructure/llm/probe.rs` (test module)
- Modify: `apps/api/Cargo.toml` (wiremock dev-dep)

- [ ] **Step 1: Add wiremock to api dev-deps**

In `apps/api/Cargo.toml`, ensure a `[dev-dependencies]` block
contains:

```toml
wiremock.workspace = true
```

If a `[dev-dependencies]` block does not exist, create one. Verify
`tokio` with the `macros` feature is already a dev-dep (it almost
certainly is — check with `grep tokio apps/api/Cargo.toml`); if
not, add `tokio = { workspace = true, features = ["macros", "rt-multi-thread"] }`.

- [ ] **Step 2: Add the failing test at the bottom of `apps/api/src/infrastructure/llm/probe.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn openai_probe_hits_supplied_base_url_with_bearer() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/embeddings"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "object": "list",
                "data": [{"object": "embedding", "index": 0, "embedding": [0.0]}],
                "model": "text-embedding-3-small",
                "usage": {"prompt_tokens": 1, "total_tokens": 1}
            })))
            .mount(&server)
            .await;

        let probe = HttpLlmProbe::default();
        probe
            .probe(LlmProvider::OpenAi, "secret-key", Some(&server.uri()))
            .await
            .expect("probe ok");

        let recv = server.received_requests().await.unwrap();
        assert_eq!(recv.len(), 1);
        assert_eq!(
            recv[0].headers.get("authorization").unwrap(),
            "Bearer secret-key"
        );
    }

    #[tokio::test]
    async fn openai_probe_no_auth_when_key_empty_and_url_set() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/embeddings"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "object": "list",
                "data": [{"object": "embedding", "index": 0, "embedding": [0.0]}],
                "model": "text-embedding-3-small",
                "usage": {"prompt_tokens": 1, "total_tokens": 1}
            })))
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
        Mock::given(method("POST"))
            .and(path("/embeddings"))
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
```

- [ ] **Step 3: Run tests — expected PASS (the implementation already exists from Task 5)**

```bash
cargo test -p historiador_api probe::tests
```

Expected: 3 tests pass.

- [ ] **Step 4: Commit**

```bash
git add apps/api/Cargo.toml apps/api/src/infrastructure/llm/probe.rs
git commit -m "test(probe): wiremock coverage for openai custom base url + no-auth"
```

---

## Task 7: `UpdateLlmConfigCommand` accepts and persists `base_url`

**Files:**
- Modify: `apps/api/src/application/admin/update_llm_config.rs`

- [ ] **Step 1: Locate or add the `#[cfg(test)] mod tests` block at the bottom of `update_llm_config.rs`. If a test infrastructure (mock workspace repo, mock probe) already exists for this file or its sibling commands, reuse it. Otherwise check `apps/api/src/application/admin/` for an existing pattern (e.g., `regenerate_token.rs` or `invite_user.rs`) and mirror it.**

```bash
grep -rn "mod tests\|MockWorkspaceRepository\|MockLlmProbe" apps/api/src/application/admin/
```

- [ ] **Step 2: Write failing tests covering the §3 matrix for OpenAI**

Add three tests to the file's test module (create one if absent).
The exact mock setup will follow whatever pattern step 1 uncovered;
the assertions are non-negotiable:

1. `provider=openai, base_url=Some, api_key=""` → use case succeeds;
   the persisted patch has `llm_base_url = Some(url)` and
   `llm_api_key_encrypted = None`.
2. `provider=openai, base_url=Some, api_key="k"` → succeeds; both
   fields persisted (key encrypted, url verbatim).
3. `provider=openai, base_url=None, api_key=""` → unchanged from
   today (key field interpreted as "don't rotate"); `base_url`
   patch field is `None`.

If no mock infrastructure exists, create
`apps/api/src/application/admin/test_doubles.rs` with minimal
`InMemoryWorkspaceRepository` and `AcceptingProbe` types (≤ 80 LoC
total) and use `#[cfg(test)] mod test_doubles;` from this file.

- [ ] **Step 3: Run the new tests — expected FAIL (`UpdateLlmConfigCommand` has no `base_url` field yet)**

```bash
cargo test -p historiador_api update_llm_config
```

- [ ] **Step 4: Modify `UpdateLlmConfigCommand` and its `execute`**

```rust
pub struct UpdateLlmConfigCommand {
    pub llm_provider: LlmProvider,
    pub llm_api_key: String,
    /// Optional. Required to be `Some` when `llm_provider == OpenAi`
    /// AND `llm_api_key` is empty (anonymous self-hosted endpoint).
    /// Persisted verbatim (after trimming) into `workspaces.llm_base_url`.
    pub base_url: Option<String>,
    pub generation_model: String,
    pub embedding_model: String,
}
```

In `execute`, replace the entire `let (encrypted_key, base_url) = match cmd.llm_provider { ... }`
block (lines 71-88) with:

```rust
let normalized_url = cmd
    .base_url
    .as_deref()
    .map(|u| u.trim().trim_end_matches('/').to_string())
    .filter(|u| !u.is_empty());

let (encrypted_key, base_url): (Option<String>, Option<String>) = match cmd.llm_provider {
    LlmProvider::Ollama => {
        // Backwards compat: Ollama still carries its URL via llm_api_key.
        if cmd.llm_api_key.is_empty() {
            (None, ws.llm_base_url.clone())
        } else {
            (None, Some(cmd.llm_api_key.trim().to_string()))
        }
    }
    LlmProvider::Test => (None, None),
    LlmProvider::Anthropic => {
        if cmd.llm_api_key.is_empty() {
            (None, None)
        } else {
            let ct = self.cipher.encrypt(&cmd.llm_api_key)?;
            (Some(ct), None)
        }
    }
    LlmProvider::OpenAi => {
        // Reject the no-url + no-key combo (would leave the workspace
        // unable to authenticate against api.openai.com).
        if normalized_url.is_none() && cmd.llm_api_key.is_empty() {
            // We allow empty key here only as "don't rotate the secret"
            // when an existing key is on file. If neither URL nor key
            // is supplied AND no key is on file, that's invalid.
            if ws.llm_api_key_encrypted.is_none() {
                return Err(ApplicationError::Domain(DomainError::Validation(
                    "openai requires either an API key or a custom base URL".into(),
                )));
            }
        }
        let encrypted = if cmd.llm_api_key.is_empty() {
            // Empty key + URL set → user intends "no auth" → wipe the stored key.
            // Empty key + URL not set → preserve existing key (don't rotate).
            if normalized_url.is_some() { None } else { ws.llm_api_key_encrypted.clone() }
        } else {
            Some(self.cipher.encrypt(&cmd.llm_api_key)?)
        };
        (encrypted, normalized_url.clone())
    }
};
```

Then update the probe call (already updated in Task 5 to accept
`None`) to pass the actual URL:

```rust
if !cmd.llm_api_key.is_empty() || normalized_url.is_some() {
    self.probe
        .probe(cmd.llm_provider, &cmd.llm_api_key, normalized_url.as_deref())
        .await
        .map_err(|e| ApplicationError::Domain(DomainError::Validation(format!("LLM rejected: {e}"))))?;
}
```

- [ ] **Step 5: Run tests — expected PASS**

```bash
cargo test -p historiador_api update_llm_config
```

- [ ] **Step 6: Commit**

```bash
git add apps/api/src/application/admin/
git commit -m "feat(admin): UpdateLlmConfigCommand accepts base_url for openai"
```

---

## Task 8: `InitializeInstallationCommand` accepts `base_url`

**Files:**
- Modify: `apps/api/src/application/setup/initialize_installation.rs`

Mirror Task 7. The setup wizard's command shape gains the same
field; the OpenAI branch uses identical persistence logic. Tests
mirror Task 7.

- [ ] **Step 1: Add failing tests** that mirror Task 7's matrix
  but call `InitializeInstallationUseCase::execute`. Reuse the same
  test doubles introduced in Task 7.

- [ ] **Step 2: Run them — expected FAIL**

```bash
cargo test -p historiador_api initialize_installation
```

- [ ] **Step 3: Add `base_url: Option<String>` to `InitializeInstallationCommand`**

Replace the OpenAI / Anthropic branch in `execute` (lines 75-82)
with the same shape as Task 7's OpenAI / Anthropic branches. The
Ollama branch and the rest of the function are unchanged.

Pass `normalized_url.as_deref()` to the probe call (line 64).

- [ ] **Step 4: Run tests — expected PASS**

```bash
cargo test -p historiador_api initialize_installation
cargo test --workspace
```

- [ ] **Step 5: Commit**

```bash
git add apps/api/src/application/setup/
git commit -m "feat(setup): InitializeInstallationCommand accepts base_url for openai"
```

---

## Task 9: Custom URL validator + apply to HTTP DTOs

**Files:**
- Create: `apps/api/src/presentation/validation.rs` (small, focused module)
- Modify: `apps/api/src/presentation/mod.rs` (re-export `validation`)
- Modify: `apps/api/src/presentation/handler/admin/workspace.rs`
- Modify: `apps/api/src/presentation/handler/setup.rs`

- [ ] **Step 1: Write the failing test for the validator**

Create `apps/api/src/presentation/validation.rs`:

```rust
//! Custom validators reused across multiple HTTP DTOs.

use validator::ValidationError;

/// Validate that a string is a usable LLM base URL per
/// spec §4: parses, http/https only, no userinfo.
/// Trailing-slash normalization happens at the application
/// layer (see `UpdateLlmConfigUseCase::execute`).
pub fn validate_llm_base_url(value: &str) -> Result<(), ValidationError> {
    let url = url::Url::parse(value).map_err(|_| ValidationError::new("invalid_url"))?;

    match url.scheme() {
        "http" | "https" => {}
        _ => return Err(ValidationError::new("scheme_must_be_http_or_https")),
    }

    if !url.username().is_empty() || url.password().is_some() {
        return Err(ValidationError::new("userinfo_not_allowed"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test] fn accepts_https() { assert!(validate_llm_base_url("https://api.example/v1").is_ok()); }
    #[test] fn accepts_http()  { assert!(validate_llm_base_url("http://localhost:8000").is_ok()); }
    #[test] fn rejects_unparseable() { assert!(validate_llm_base_url("not a url").is_err()); }
    #[test] fn rejects_ftp() { assert!(validate_llm_base_url("ftp://example.com").is_err()); }
    #[test] fn rejects_userinfo() {
        assert!(validate_llm_base_url("https://user:pass@example.com").is_err());
        assert!(validate_llm_base_url("https://user@example.com").is_err());
    }
    #[test] fn accepts_with_trailing_slash() {
        assert!(validate_llm_base_url("https://api.example/v1/").is_ok());
    }
}
```

Add `pub mod validation;` to `apps/api/src/presentation/mod.rs`.

If `url` is not yet a direct dep of `apps/api`, add
`url.workspace = true` to `apps/api/Cargo.toml` (it is almost
certainly already a transitive dep — confirm with `cargo tree -p historiador_api -i url`).

- [ ] **Step 2: Run the validator tests — expected PASS**

```bash
cargo test -p historiador_api validation::tests
```

- [ ] **Step 3: Apply the validator to `LlmPatchRequest`**

In `apps/api/src/presentation/handler/admin/workspace.rs`, modify
`LlmPatchRequest`:

```rust
#[derive(Debug, Deserialize, Validate, utoipa::ToSchema)]
pub struct LlmPatchRequest {
    pub llm_provider: LlmProvider,
    /// API key for cloud providers or base URL for Ollama. Leave
    /// empty to keep the existing secret (useful when editing only
    /// the model names) or — for the OpenAI provider with a custom
    /// `base_url` — to mark the endpoint as unauthenticated.
    #[validate(length(max = 512))]
    #[serde(default)]
    pub llm_api_key: String,

    /// Optional OpenAI-compatible base URL (e.g.
    /// `https://litellm.example/v1`). Persisted verbatim into
    /// `workspaces.llm_base_url`. When set together with an empty
    /// `llm_api_key`, no Authorization header is sent.
    #[validate(length(max = 512))]
    #[validate(custom(function = "crate::presentation::validation::validate_llm_base_url"))]
    #[serde(default)]
    pub base_url: Option<String>,

    #[validate(length(min = 1, max = 128))]
    pub generation_model: String,
    #[validate(length(min = 1, max = 128))]
    pub embedding_model: String,
}
```

In the handler body (around line 174), pass `body.base_url` to
`UpdateLlmConfigCommand`:

```rust
UpdateLlmConfigCommand {
    llm_provider: body.llm_provider,
    llm_api_key: body.llm_api_key,
    base_url: body.base_url,
    generation_model: body.generation_model,
    embedding_model: body.embedding_model,
}
```

- [ ] **Step 4: Apply the validator to `SetupRequest` and `ProbeRequest`**

In `apps/api/src/presentation/handler/setup.rs`, add the same
optional `base_url` field to both DTOs (both with the same
`#[validate(custom...)]` annotation), and plumb it through to
`InitializeInstallationCommand` (line ~78) and
`ProbeLlmUseCase::execute` (line 154).

For the probe handler, the call site becomes:

```rust
.execute(body.llm_provider, &body.llm_api_key, body.base_url.as_deref())
```

- [ ] **Step 5: Run the workspace test suite**

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Expected: green. (The validator's `Option<String>` annotation
through `validator` v0.19+ runs the custom function only when
`Some`.)

- [ ] **Step 6: Commit**

```bash
git add apps/api/src/presentation/
git commit -m "feat(api): base_url field on LLM HTTP DTOs with shared URL validator"
```

---

## Task 10: Boot-time client construction reads `llm_base_url`

**Files:**
- Modify: `apps/api/src/main.rs` (`build_llm_clients_from_workspace`, lines 295-380 approximately)

The Ollama branch already reads `ws.llm_base_url`; only the
`openai` and `anthropic` (which uses an OpenAI embedding client)
arms need the change.

- [ ] **Step 1: Refactor the OpenAI arm**

Locate the `"openai"` arm (around line 339-351). Replace with:

```rust
"openai" => {
    let key_str = ws
        .llm_api_key_encrypted
        .as_deref()
        .map(|k| cipher.decrypt(k))
        .transpose()?
        .unwrap_or_default();
    let key: Option<&str> = if key_str.is_empty() { None } else { Some(&key_str) };
    let base_url = ws.llm_base_url.as_deref();
    tracing::info!(
        gen = gen_model,
        embed = embed_model,
        base_url = base_url.unwrap_or("api.openai.com"),
        "LLM provider: OpenAI"
    );
    (
        Arc::new(OpenAiEmbeddingClient::from_config(OpenAiEmbeddingConfig {
            api_key: key,
            base_url,
            model: embed_model,
            dim: 1536,
        })),
        Arc::new(OpenAiTextGenerationClient::from_config(OpenAiGenerationConfig {
            api_key: key,
            base_url,
            model: gen_model,
        })),
    )
}
```

- [ ] **Step 2: Refactor the Anthropic arm's embedding client**

Anthropic-with-OpenAI-embeddings reads the embedding key from the
`EMBEDDING_API_KEY` env var (line 357-364). For consistency, also
honor `ws.llm_base_url` here for the embedding client only:

```rust
"anthropic" => {
    let key = ws
        .llm_api_key_encrypted
        .as_deref()
        .map(|k| cipher.decrypt(k))
        .transpose()?
        .unwrap_or_default();
    let emb: Arc<dyn EmbeddingClient> = match std::env::var("EMBEDDING_API_KEY") {
        Ok(k) if !k.is_empty() => {
            Arc::new(OpenAiEmbeddingClient::from_config(OpenAiEmbeddingConfig {
                api_key: Some(&k),
                base_url: ws.llm_base_url.as_deref(),
                model: embed_model,
                dim: 1536,
            }))
        }
        _ => Arc::new(StubEmbeddingClient::default()),
    };
    tracing::info!(gen = gen_model, "LLM provider: Anthropic");
    (
        emb,
        Arc::new(AnthropicTextGenerationClient::with_model(&key, gen_model)),
    )
}
```

Update the `use historiador_llm::...` import line at the top of
`main.rs` to include `OpenAiEmbeddingConfig, OpenAiGenerationConfig`.

- [ ] **Step 3: Verify the workspace builds and tests pass**

```bash
cargo build --workspace
cargo test --workspace
```

- [ ] **Step 4: Commit**

```bash
git add apps/api/src/main.rs
git commit -m "feat(api): build_llm_clients_from_workspace honors llm_base_url for openai"
```

---

## Task 11: Regenerate OpenAPI + TypeScript types

**Files:**
- Modify: `openapi.yaml` (regenerated)
- Modify: `packages/types/generated/index.ts` (regenerated)

- [ ] **Step 1: Regenerate**

```bash
pnpm gen:types
```

- [ ] **Step 2: Inspect the diff**

```bash
git diff openapi.yaml packages/types/generated/index.ts
```

Expected: only the `base_url` field appears, on `LlmPatchRequest`,
`SetupRequest`, and `ProbeRequest` (or whatever names utoipa
emits — match against the Rust struct names). No spurious diff in
unrelated schemas.

- [ ] **Step 3: Commit**

```bash
git add openapi.yaml packages/types/generated/index.ts
git commit -m "chore(types): regen openapi + ts types for base_url field"
```

---

## Task 12: Frontend service types

**Files:**
- Modify: `apps/web/lib/services/admin.ts`
- Modify: `apps/web/lib/services/setup.ts` (if it exists; locate via
  `grep -rn "setup" apps/web/lib/services/`)

The auto-generated types in `@historiador/types` already carry
`base_url` (from Task 11). The hand-written `LlmPatchBody` in
`admin.ts` is the wrapper most components import — extend it.

- [ ] **Step 1: Locate and inspect**

```bash
cat apps/web/lib/services/admin.ts | head -80
```

- [ ] **Step 2: Add the optional field**

```ts
export interface LlmPatchBody {
  llm_provider: "openai" | "anthropic" | "ollama" | "test";
  llm_api_key?: string;
  base_url?: string;
  generation_model: string;
  embedding_model: string;
}
```

If a sibling `SetupBody` interface exists in `setup.ts`, give it
the same field.

- [ ] **Step 3: Verify the workspace lints**

```bash
cd apps/web && pnpm lint
```

- [ ] **Step 4: Commit**

```bash
git add apps/web/lib/services/
git commit -m "feat(web): LlmPatchBody + SetupBody accept optional base_url"
```

---

## Task 13: Admin LLM settings form — add base URL field

**Files:**
- Modify: `apps/web/components/admin/llm-settings-form.tsx`

Re-read `apps/web/AGENTS.md` and any relevant Next.js 16 docs in
`apps/web/node_modules/next/dist/docs/` before touching the file.

The form currently has a shared
`renderApiKeyProviderForm(label: "OpenAI" | "Anthropic")` helper
that renders an API-key input + "Test connection" button. The URL
field belongs only to OpenAI mode, so the helper needs to know
which provider is active.

- [ ] **Step 1: Add a `baseUrl` controlled state**

Insert near the other `useState` declarations (around line 47-58):

```tsx
const [baseUrl, setBaseUrl] = useState<string>(workspace.llm_base_url ?? "");
```

- [ ] **Step 2: Extend the shared OpenAI/Anthropic renderer**

Replace the existing `renderApiKeyProviderForm` (lines 217-250)
with:

```tsx
const renderApiKeyProviderForm = (label: "OpenAI" | "Anthropic") => {
  const isOpenAi = label === "OpenAI";
  // Test connection is enabled when EITHER a key is present OR
  // (for OpenAI only) a base URL is set — covers the no-auth path.
  const canTest = !!apiKey.trim() || (isOpenAi && !!baseUrl.trim());
  return (
    <div className="space-y-4">
      <Input
        label={`${label} API key (leave empty to keep${
          isOpenAi ? " or to use no auth with a custom URL" : ""
        })`}
        type="password"
        value={apiKey}
        onChange={(e) => setApiKey(e.target.value)}
        placeholder="•••••••• (keep existing)"
      />
      {isOpenAi && (
        <Input
          label="Base URL (optional)"
          type="url"
          value={baseUrl}
          onChange={(e) => setBaseUrl(e.target.value)}
          placeholder="https://api.openai.com/v1 (default)"
        />
      )}
      <Button
        variant="secondary"
        size="sm"
        onClick={testConnection}
        disabled={testing || !canTest}
      >
        {testing ? <><Spinner className="mr-2" /> Testing…</> : "Test connection"}
      </Button>
      {probeBlock}
      <div className="grid grid-cols-2 gap-4">
        <Input
          label="Generation model"
          value={generationModel}
          onChange={(e) => setGenerationModel(e.target.value)}
        />
        <Input
          label="Embedding model"
          value={embeddingModel}
          onChange={(e) => setEmbeddingModel(e.target.value)}
        />
      </div>
      {saveBlock}
      {resultBlock}
    </div>
  );
};
```

- [ ] **Step 3: Pass `base_url` in the `updateLlmConfig` call (line ~140-145)**

Locate the `await adminService.updateLlmConfig({...})` call (around
line 140) and insert the `base_url` field:

```tsx
await adminService.updateLlmConfig({
  llm_provider: provider,
  llm_api_key: apiKey,
  base_url: provider === "openai" && baseUrl.trim() !== "" ? baseUrl.trim() : undefined,
  generation_model: generationModel,
  embedding_model: embeddingModel,
});
```

- [ ] **Step 3b: Pass `base_url` in the `testConnection` probe call**

Find the `testConnection` function (it calls `setupService.probeLlm`
or similar — search for `probe` in the file). Pass `base_url` the
same way.

- [ ] **Step 4: Verify lint + dev server**

```bash
cd apps/web && pnpm lint
# In a separate terminal, with the API + DB running:
pnpm dev
```

Manually visit `http://localhost:3000/dashboard/admin`, change the
provider to OpenAI, fill in a fake base URL, and confirm the form
submits the new field (network tab should show `base_url` in the
PATCH body). Don't expect the probe to succeed against a fake URL —
the goal here is to verify the form wiring.

- [ ] **Step 5: Commit**

```bash
git add apps/web/components/admin/llm-settings-form.tsx
git commit -m "feat(web): admin LLM settings — base URL field for openai provider"
```

---

## Task 14: Setup wizard — add base URL field

**Files:**
- Modify: `apps/web/app/setup/page.tsx`

- [ ] **Step 1: Locate the OpenAI section of the wizard**

```bash
grep -n "openai\|llm_provider\|llm_api_key" apps/web/app/setup/page.tsx
```

The wizard already references `llmProvider` and `llmApiKey` state
(lines 131, 132, 183, 184 from the earlier read). Mirror the
`useState` and JSX additions from Task 13.

- [ ] **Step 2: Add `baseUrl` state + conditional input + payload field**

```tsx
const [baseUrl, setBaseUrl] = useState("");

// ... in JSX, under the API-key field, gated on llmProvider === "openai" ...

// In both /setup/init and /setup/probe call sites, add:
//   base_url: llmProvider === "openai" && baseUrl.trim() !== "" ? baseUrl.trim() : undefined,
```

- [ ] **Step 3: Verify lint + visit `/setup` to test the wizard end-to-end against the test provider**

```bash
cd apps/web && pnpm lint
```

Restart the API binary with a fresh DB
(`docker compose down -v && docker compose up -d`) and walk through
the wizard with `provider=test` to confirm nothing regressed.

- [ ] **Step 4: Commit**

```bash
git add apps/web/app/setup/page.tsx
git commit -m "feat(web): setup wizard — base URL field for openai provider"
```

---

## Task 15: CHANGELOG entry

**Files:**
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Add entry under the next release's `## Added` section**

Open `CHANGELOG.md` and add (under the "Unreleased" or next
version's `### Added`):

```markdown
- OpenAI provider now accepts an optional custom base URL and an
  optional API key, enabling self-hosted OpenAI-compatible
  endpoints (vLLM, LiteLLM, OpenRouter, LM Studio, Azure-via-proxy).
  Configured per-workspace via the setup wizard or admin LLM
  config form. (#TODO-pr-number)
```

- [ ] **Step 2: Commit**

```bash
git add CHANGELOG.md
git commit -m "docs(changelog): note custom base URL for OpenAI provider"
```

---

## Task 16: End-to-end smoke against a local mock

This is a manual verification step, not a TDD cycle. It's the
last gate before opening a PR.

- [ ] **Step 1: Stand up a wiremock-driven OpenAI mock** (or use
  any local OpenAI-compatible server you have handy — LiteLLM,
  vLLM, LM Studio):

```bash
# Quickest path: a one-shot script with httpmock-style stubbing.
# Or run LiteLLM:
docker run --rm -p 8000:8000 ghcr.io/berriai/litellm:main-latest \
  --model gpt-4o-mini --port 8000
```

- [ ] **Step 2: Walk through the admin LLM config form**

1. Log in as the workspace admin at `http://localhost:3000`.
2. Navigate to `/dashboard/admin`.
3. Set provider = OpenAI, base URL = `http://localhost:8000`,
   API key = (whatever the mock requires; or empty for an
   unauthenticated mock).
4. Click "Test connection" and confirm success.
5. Submit. Confirm the workspace row in PostgreSQL has the new
   `llm_base_url`:

```bash
psql "$DATABASE_URL_READWRITE" \
  -c "SELECT llm_provider, llm_base_url, generation_model, embedding_model FROM workspaces;"
```

- [ ] **Step 3: Trigger an actual embedding call**

Edit any draft page and click "Publish". The publish path embeds
chunks; check API logs to confirm the OpenAI client hit
`http://localhost:8000` rather than `api.openai.com`.

- [ ] **Step 4: Confirm `requires_reindex` plumbing still works**

Change `embedding_model` and submit. The form should report
"Reindex required" with a non-zero affected page count.

- [ ] **Step 5: Final cargo + pnpm green check**

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
cd apps/web && pnpm lint
```

Expected: all green.

- [ ] **Step 6: Open the PR**

```bash
gh pr create --base develop --title "feat: custom base URL for OpenAI provider" \
  --body "$(cat <<'EOF'
## Summary
- Adds an optional per-workspace base URL on the OpenAI provider, enabling vLLM / LiteLLM / OpenRouter / LM Studio / Azure-via-proxy targets.
- Adds optional auth: when the URL is set and the API key is blank, no Authorization header is sent (for self-hosted servers without auth).
- Custom `OpenAiCompatConfig` implements `async-openai`'s `Config` trait so we get conditional headers without forking the SDK.

## Test plan
- [ ] `cargo test --workspace` green
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` green
- [ ] `cd apps/web && pnpm lint` green
- [ ] Manual setup-wizard walkthrough against a local LiteLLM mock
- [ ] Manual admin-form walkthrough: rotate provider, hit "Test connection", submit, publish a page, verify the embedding call hits the custom URL

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

## Self-review

After Task 16, walk through the spec section-by-section:

- §1 Goal — covered by Tasks 2-10.
- §2 Non-goals — none of them sneaked into the plan; pre-setup
  `OPENAI_BASE_URL` env var is explicitly omitted (per spec).
- §3 Configuration semantics matrix — Task 7 step 4 implements the
  five OpenAI rows; Task 8 mirrors for setup; the Ollama, Anthropic,
  and Test rows are left untouched.
- §4 URL validation rules — Task 9 covers parse, scheme, and
  userinfo; trailing-slash normalization happens in
  `OpenAiCompatConfig::new` (Task 2) and again in
  `update_llm_config.rs` (Task 7 step 4).
- §5 Components changed — every numbered subsection maps to a task:
  5.1 → Tasks 2-4, 5.2 → Tasks 5-6, 5.3 → Tasks 7-8, 5.4 → Task 9,
  5.5 → Task 10, 5.6 → Tasks 12-14.
- §6 Data flow — verified manually in Task 16.
- §7 Error handling — Task 6 covers probe transport / 4xx; Task 7
  validation rejects the no-URL + no-key combo; Task 9 rejects
  invalid URLs; Task 9's userinfo rule covers the credential-leak
  case.
- §8 Testing — every subsection (8.1 → Tasks 3-4, 8.2 → Tasks 7-8,
  8.3 → Task 6, 8.4 → covered indirectly by the use-case tests, 8.5
  → Task 11, 8.6 → Tasks 13-14) is represented.
- §9 Migration & rollout — no migration; Task 15 adds the changelog
  entry; Task 16 confirms default behavior unchanged.
- §10 Open implementation question — decided in Task 2 (custom
  `Config` impl).
