# EmbeddingClient Retirement — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the vestigial `EmbeddingClient` from the runtime. Chronik handles embeddings server-side (configured at the topic level, with its own `OPENAI_API_KEY` env on the Chronik container); the app's `EmbeddingClient` trait, OpenAI/Ollama/Stub embedding impls, the `embedding_client` field on `AppState`, the `workspaces.embedding_model` column, the `requires_reindex` UX banner, and the `EMBEDDING_API_KEY` app-side env-var convention are all dead code from the pre-Chronik (VexFS) era. This plan removes them and fixes the OpenAI probe to validate URL+auth via `GET {base}/models` instead of an embedding call.

**Architecture:** Cut top-down — drop from `AppState` first (no production readers), then retire the trait and impls in `historiador_llm`, then strip `embedding_model` from the use cases / DTOs / repository / Workspace struct, then drop the database column with a forward-only migration, then trim the admin form and setup wizard. The OpenAI probe switches to the model-listing endpoint (no hardcoded model name). The base-URL feature shipped on this branch is preserved — `OpenAiCompatConfig` is shared with the text-generation client and survives.

**Tech Stack:** Rust (Axum, sqlx, async-openai), Next.js 16 / React 19, TypeScript, pnpm, turborepo. No new dependencies.

**Spec:** This plan stands as both spec and plan; design decisions are called out below. The base-URL feature spec at `artifacts/specs/2026-05-01-openai-custom-base-url-design.md` remains the contract for the URL/auth path.

---

## Design decisions

These are the calls baked into the plan. If any is wrong for the project, override before execution.

- **D1 — Probe behavior.** OpenAI probe switches from a `POST /embeddings` call to `GET {base}/models`. No model name in the request. Validates URL + auth + protocol shape in one cheap call. Anthropic and Ollama probe arms are unchanged.
- **D2 — Chronik embedding model config.** The hardcoded `text-embedding-3-small` in `crates/db/src/chronik/kafka_producer.rs::published_pages_topic_config` stays. It's a deployment-level concern (Chronik topic config), not a workspace-level one. A future PR can promote it to an env var if needed; out of scope here.
- **D3 — `EMBEDDING_API_KEY` env var.** Stays in `docker-compose.yml` (Chronik container reads it). Removed from the app's pre-setup env-var fallback path. The `.env.example` is updated to clarify this is Chronik-only.
- **D4 — `requires_reindex` UX.** The banner ("Changing the embedding model requires re-embedding X pages") goes away — there's no longer an embedding-model knob to change. The manual "Reindex" button on the admin form stays as an emergency operator tool. The `LlmPatchResponse.requires_reindex` field is dropped from the API.
- **D5 — Migration.** Forward-only `ALTER TABLE workspaces DROP COLUMN embedding_model`. The codebase has no rollback migrations. Existing workspace rows lose the column without ceremony.
- **D6 — Anthropic-with-OpenAI-embeddings sidecar.** The code path in `build_llm_clients_from_workspace` that built an `OpenAiEmbeddingClient` from `EMBEDDING_API_KEY` for Anthropic-mode workspaces goes away entirely.
- **D7 — `OpenAiCompatConfig` survives.** Used by the text-generation client. Only the embedding-specific refactor from the in-flight base-URL PR (`OpenAiEmbeddingClient::from_config`, `OpenAiEmbeddingConfig`) gets removed alongside the rest of `EmbeddingClient`.
- **D8 — Branch strategy.** This plan lands on `feature/accept-url-openai`. The base-URL PR is now a *combined* base-URL-and-retirement PR. The combined PR title should reflect both changes. Alternative: cut a fresh branch off develop, ship the base-URL PR first, then this. Recommendation is the combined route — the retirement reverts some of the in-flight Tasks 2-3 work and shipping that work only to delete it would be wasteful.

---

## Pre-flight

- [ ] **Confirm branch + clean tree**

```bash
git status
git rev-parse --abbrev-ref HEAD
```

Expected: clean tree, branch `feature/accept-url-openai`.

- [ ] **Confirm baseline green**

```bash
cargo build --workspace
cargo test --workspace --lib --bins
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
```

Expected: all green.

- [ ] **Re-read the Chronik rewrite plan** at
  `artifacts/plans/2026-04-25-chronik-vector-store-rewrite.md` — its
  Task 1 explicitly removes `EmbeddingClient` from the chunk pipeline
  and MCP search. This plan finishes the job.

---

## Task 1: OpenAI probe → `GET {base}/models`

The probe is the only production consumer of `OpenAiEmbeddingClient` — switching it to a non-embedding endpoint unblocks every later task. Anthropic and Ollama arms stay as-is. The wiremock tests in `apps/api/src/infrastructure/llm/probe.rs` are rewritten to mock `GET /models`.

**Files:**
- Modify: `apps/api/src/infrastructure/llm/probe.rs`

- [ ] **Step 1: Replace the OpenAI probe arm**

Locate the `LlmProvider::OpenAi` arm in `HttpLlmProbe::probe` (added in Task 5 of the base-URL plan, around line 75-99). Replace it with:

```rust
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
```

- [ ] **Step 2: Drop the now-unused embedding-client import**

Inside the same `match` block, the `LlmProvider::OpenAi` arm previously imported `historiador_llm::{EmbeddingClient, OpenAiEmbeddingClient, OpenAiEmbeddingConfig}` locally. Remove that `use` statement.

- [ ] **Step 3: Rewrite the three wiremock tests**

Replace the existing `#[cfg(test)] mod tests` block at the bottom of `apps/api/src/infrastructure/llm/probe.rs` with the new shape:

```rust
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
```

- [ ] **Step 4: Verify**

```bash
cargo test -p historiador_api probe::tests
cargo build --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
```

Expected: 3 probe tests pass; rest of the workspace stays green.

- [ ] **Step 5: Commit**

```bash
git add apps/api/src/infrastructure/llm/probe.rs
git commit -m "refactor(probe): openai probe uses GET /models, no hardcoded model"
```

---

## Task 2: Drop `embedding_client` from `AppState`

Two AppState files carry the `embedding_client: Arc<dyn EmbeddingClient>` field — `apps/api/src/state.rs` and `apps/api/src/presentation/state.rs`. No production code reads either. Remove both.

**Files:**
- Modify: `apps/api/src/state.rs`
- Modify: `apps/api/src/presentation/state.rs`
- Modify: `apps/api/src/main.rs`

- [ ] **Step 1: Drop the field from `apps/api/src/state.rs`**

Remove the `pub embedding_client: Arc<dyn EmbeddingClient>,` field (around line 35) and its doc comment. Remove `EmbeddingClient` from the `use historiador_llm::{...}` import line. Adjust any constructor / field initializer that referenced `embedding_client`.

- [ ] **Step 2: Drop the field from `apps/api/src/presentation/state.rs`**

Same shape: remove the `pub embedding_client: Arc<dyn EmbeddingClient>,` field (around line 64), the `EmbeddingClient` import (line 17), and any constructor reference.

- [ ] **Step 3: Drop the wire-up in `apps/api/src/main.rs`**

In `main` (around line 113-244):
- The destructuring `let (embedding_client, text_generation_client) = build_llm_clients_from_workspace(...)?;` becomes `let text_generation_client = build_llm_clients_from_workspace(...)?;`.
- The two AppState construction sites (`embedding_client: embedding_client.clone(),` at line 230; `embedding_client,` at line 244) drop those lines.
- `build_llm_clients_from_workspace`'s return type changes — see Task 3.

- [ ] **Step 4: Verify**

```bash
cargo build --workspace
```

Expected: builds clean. `cargo test` may have unrelated failures from later cleanup tasks — this step only needs the build to succeed.

- [ ] **Step 5: Commit**

```bash
git add apps/api/src/state.rs apps/api/src/presentation/state.rs apps/api/src/main.rs
git commit -m "refactor(state): drop embedding_client from AppState (chronik owns embeddings)"
```

---

## Task 3: Simplify `build_llm_clients_from_workspace`

**Files:**
- Modify: `apps/api/src/main.rs`

- [ ] **Step 1: Change signature and trim arms**

Replace `build_llm_clients_from_workspace` (around lines 295-400) with:

```rust
/// Build the text-generation client from a workspace row, falling
/// back to env vars before setup has completed. Embeddings are
/// handled by Chronik server-side; the app no longer constructs an
/// embedding client.
fn build_llm_clients_from_workspace(
    cipher: &Cipher,
    workspace: Option<&historiador_db::postgres::workspaces::Workspace>,
) -> anyhow::Result<Arc<dyn TextGenerationClient>> {
    // Pre-setup: honor legacy LLM_PROVIDER + LLM_API_KEY env vars.
    let Some(ws) = workspace else {
        let provider = std::env::var("LLM_PROVIDER").unwrap_or_default();
        let api_key = std::env::var("LLM_API_KEY").unwrap_or_default();
        return Ok(match provider.as_str() {
            "openai" if !api_key.is_empty() => {
                tracing::info!("LLM provider (env): OpenAI");
                Arc::new(OpenAiTextGenerationClient::new(&api_key))
            }
            "anthropic" if !api_key.is_empty() => {
                tracing::info!("LLM provider (env): Anthropic");
                Arc::new(AnthropicTextGenerationClient::new(&api_key))
            }
            _ => {
                tracing::info!("LLM provider (env): stub — setup not complete");
                Arc::new(StubTextGenerationClient)
            }
        });
    };

    let gen_model = ws.generation_model.as_str();

    let client: Arc<dyn TextGenerationClient> = match ws.llm_provider.as_str() {
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
                base_url = base_url.unwrap_or("api.openai.com"),
                "LLM provider: OpenAI"
            );
            Arc::new(OpenAiTextGenerationClient::from_config(
                OpenAiGenerationConfig {
                    api_key: key,
                    base_url,
                    model: gen_model,
                },
            ))
        }
        "anthropic" => {
            let key = ws
                .llm_api_key_encrypted
                .as_deref()
                .map(|k| cipher.decrypt(k))
                .transpose()?
                .unwrap_or_default();
            tracing::info!(gen = gen_model, "LLM provider: Anthropic");
            Arc::new(AnthropicTextGenerationClient::with_model(&key, gen_model))
        }
        "ollama" => {
            let base_url = ws
                .llm_base_url
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("ollama provider requires llm_base_url"))?;
            tracing::info!(gen = gen_model, base_url, "LLM provider: Ollama");
            Arc::new(OllamaTextClient::new(base_url, gen_model))
        }
        _ => {
            tracing::info!(provider = ws.llm_provider.as_str(), "LLM provider: stub");
            Arc::new(StubTextGenerationClient)
        }
    };

    Ok(client)
}
```

- [ ] **Step 2: Trim the `historiador_llm::{...}` import line at the top of `main.rs`**

Remove `EmbeddingClient`, `OllamaEmbeddingClient`, `OpenAiEmbeddingClient`, `OpenAiEmbeddingConfig`, `StubEmbeddingClient`. Keep `OllamaTextClient`, `OpenAiGenerationConfig`, `OpenAiTextGenerationClient`, `AnthropicTextGenerationClient`, `StubTextGenerationClient`, `TextGenerationClient`.

- [ ] **Step 3: Verify**

```bash
cargo build --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Expected: clean. Tests that previously asserted on `embedding_client` may fail; that's addressed in later tasks.

- [ ] **Step 4: Commit**

```bash
git add apps/api/src/main.rs
git commit -m "refactor(boot): build_llm_clients_from_workspace returns text-gen client only"
```

---

## Task 4: Retire the `EmbeddingClient` trait and all impls

**Files:**
- Modify: `crates/llm/src/lib.rs`
- Delete (or empty): nothing — the trait + impls live inline in their sibling modules
- Modify: `crates/llm/src/openai.rs` — drop `OpenAiEmbeddingClient` + `OpenAiEmbeddingConfig` + their tests
- Modify: `crates/llm/src/ollama.rs` — drop `OllamaEmbeddingClient` + its tests
- Modify: `crates/llm/src/stub.rs` — drop `StubEmbeddingClient` + its tests
- Modify: `crates/llm/src/anthropic.rs` — drop the `EmbeddingClient` import if any (anthropic.rs has no embedding impl, but may import the trait)
- Modify (if exists): the `Embedding` struct definition + `EmbeddingClient` trait declaration in `lib.rs`

- [ ] **Step 1: Locate the trait and `Embedding` struct**

```bash
grep -n "pub trait EmbeddingClient\|pub struct Embedding\b" crates/llm/src/lib.rs crates/llm/src/*.rs
```

The trait + `Embedding` struct likely live near the top of `crates/llm/src/lib.rs` (the embedding module). Note their exact location.

- [ ] **Step 2: Remove the trait + struct from `lib.rs`**

Delete the `pub trait EmbeddingClient` declaration and the `pub struct Embedding` (with `vector: Vec<f32>`) definition. Drop the `pub use ...EmbeddingClient...` re-exports near the bottom of `lib.rs`. Remove `pub use stub::StubEmbeddingClient`, `pub use ollama::{OllamaEmbeddingClient, ...}`, `pub use openai::{OpenAiEmbeddingClient, OpenAiEmbeddingConfig}`. Keep the text-gen / tool-calling / `OpenAiCompatConfig` / `OpenAiGenerationConfig` re-exports.

- [ ] **Step 3: Trim `openai.rs`**

Remove the `OpenAiEmbeddingConfig` struct (currently lines ~19-28), the `OpenAiEmbeddingClient` struct (lines ~30-37), its `impl` block with `new`/`with_model`/`from_config` (lines ~39-61), and the `impl EmbeddingClient for OpenAiEmbeddingClient` block (lines ~63-101). Remove the two embedding wiremock tests from the `mod tests` block (`embed_with_custom_base_url_and_bearer`, `embed_without_authorization_header`). The `chat_with_custom_base_url_and_no_auth` test stays.

Also drop `CreateEmbeddingRequestArgs` from the `use async_openai::types::{...}` import (it's no longer used).

The text-gen client + `OpenAiGenerationConfig` + `OpenAiCompatConfig` import + the `mod openai_compat` re-export stay untouched.

- [ ] **Step 4: Trim `ollama.rs`**

Remove the `OllamaEmbeddingClient` struct, its `impl` block (around lines 267-336), and the `impl EmbeddingClient for OllamaEmbeddingClient` block (around lines 338-360 — body shape may vary). Remove the `embedding_round_trip_and_dim_cache` test (around line 440-455) from the integration-test `mod`. The `OllamaTextClient` and `list_models` function stay.

- [ ] **Step 5: Trim `stub.rs`**

Remove the `StubEmbeddingClient` struct (around line 19), its `Default` impl, and the `impl EmbeddingClient for StubEmbeddingClient` block. Remove its tests (the ones that construct `StubEmbeddingClient`). The `StubTextGenerationClient` and `StubToolCallingClient` stay.

- [ ] **Step 6: Trim `anthropic.rs`**

```bash
grep -n "EmbeddingClient\|Embedding" crates/llm/src/anthropic.rs
```

If `EmbeddingClient` or the `Embedding` struct are imported, drop them. (Anthropic doesn't implement embedding; the import is leftover.)

- [ ] **Step 7: Verify**

```bash
cargo build -p historiador_llm
cargo test -p historiador_llm
cargo clippy -p historiador_llm --all-targets --all-features -- -D warnings
```

Expected: builds clean, tests pass (text-gen + tool-calling + `OpenAiCompatConfig` tests).

- [ ] **Step 8: Verify the workspace**

```bash
cargo build --workspace
```

Expected: builds clean. Use cases / DTOs that still reference `embedding_model: String` will compile because that's an in-app string field, not the trait. They get cleaned up in later tasks.

- [ ] **Step 9: Commit**

```bash
git add crates/llm/src/
git commit -m "refactor(llm): retire EmbeddingClient trait + OpenAI/Ollama/Stub impls"
```

---

## Task 5: Drop `embedding_model` from `UpdateLlmConfigUseCase`

**Files:**
- Modify: `apps/api/src/application/admin/update_llm_config.rs`

- [ ] **Step 1: Drop the field and the embedding-changed branch**

Remove `pub embedding_model: String,` from `UpdateLlmConfigCommand` (around line 22). Remove the `pub requires_reindex: bool` field from `UpdateLlmConfigResult` (around line 26) along with `affected_page_versions` and `requires_restart` if those are also embedding-driven (verify by reading the use case body — `requires_restart` is tied to `generation_changed`, keep it).

In `execute`:
- Remove the `embedding_changed` calculation (line 143).
- Remove the `affected_page_versions` calculation (lines 144-152).
- Remove `requires_reindex: embedding_changed && affected_page_versions > 0,` from the `Ok(UpdateLlmConfigResult { ... })` literal (line 157).
- Remove `affected_page_versions` from the result.
- Remove `embedding_model: cmd.embedding_model.clone(),` from the `LlmConfigPatch { ... }` struct literal (line 138).

The result struct is now `UpdateLlmConfigResult { requires_restart: bool }`.

- [ ] **Step 2: Update the unit tests**

Each of the 6 tests (lines ~195-360) constructs `UpdateLlmConfigCommand { ... embedding_model: "text-embedding-3-small".to_string(), ... }`. Remove that field from each test call site. Remove any `assert_eq!(result.requires_reindex, ...)` or `assert_eq!(result.affected_page_versions, ...)` assertions.

- [ ] **Step 3: Verify**

```bash
cargo test -p historiador_api admin::update_llm_config
```

Expected: 6 tests pass.

- [ ] **Step 4: Commit**

```bash
git add apps/api/src/application/admin/update_llm_config.rs
git commit -m "refactor(admin): drop embedding_model from UpdateLlmConfigCommand"
```

---

## Task 6: Drop `embedding_model` from `InitializeInstallationUseCase`

**Files:**
- Modify: `apps/api/src/application/setup/initialize_installation.rs`

Mirror Task 5 for the setup wizard's command.

- [ ] **Step 1: Drop the field**

Remove `pub embedding_model: Option<String>` (or `String`, depending on shape — verify) from `InitializeInstallationCommand`. Remove the field from the workspace insert/initialize call site inside `execute`.

- [ ] **Step 2: Update tests**

Update the 6 unit tests (the ones added in Task 8 of the base-URL plan) to drop `embedding_model` from their command construction.

- [ ] **Step 3: Verify**

```bash
cargo test -p historiador_api setup::initialize_installation
```

Expected: 6 tests pass.

- [ ] **Step 4: Commit**

```bash
git add apps/api/src/application/setup/initialize_installation.rs
git commit -m "refactor(setup): drop embedding_model from InitializeInstallationCommand"
```

---

## Task 7: Drop `embedding_model` from HTTP DTOs

**Files:**
- Modify: `apps/api/src/presentation/handler/admin/workspace.rs` (`LlmPatchRequest`, `LlmPatchResponse`)
- Modify: `apps/api/src/presentation/handler/setup.rs` (`SetupRequest`, possibly `SetupResponse`)

- [ ] **Step 1: Trim `LlmPatchRequest`**

Remove `pub embedding_model: String,` (around line 34) and its `#[validate]` attributes. The handler that constructs `UpdateLlmConfigCommand` from `body` (around line 174) drops `embedding_model: body.embedding_model,`.

- [ ] **Step 2: Trim `LlmPatchResponse`**

Remove `requires_reindex: bool` and `affected_page_versions: i64` (around lines 142-145). The handler returning the response drops both fields. Keep `success: bool` and `requires_restart: bool`.

- [ ] **Step 3: Trim `WorkspaceResponse` (admin GET endpoint)**

`apps/api/src/presentation/handler/admin/workspace.rs` exposes a `WorkspaceResponse` (around line 30-90) that includes `embedding_model: String`. Remove that field and its assignment from the response builder. The frontend admin form (Task 11) stops reading it.

- [ ] **Step 4: Trim `SetupRequest` and `SetupResponse`**

Remove `pub embedding_model: Option<String>,` (around line 41) and its validate attrs from `SetupRequest`. The handler dropping `embedding_model: body.embedding_model,` from the command construction. If `SetupResponse` exposes it, remove there too.

- [ ] **Step 5: Verify**

```bash
cargo build --workspace
cargo test -p historiador_api --lib
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Expected: green.

- [ ] **Step 6: Commit**

```bash
git add apps/api/src/presentation/handler/
git commit -m "refactor(api): drop embedding_model from LLM HTTP DTOs"
```

---

## Task 8: Drop `embedding_model` from `WorkspaceRepository` + queries

**Files:**
- Modify: `apps/api/src/domain/port/workspace_repository.rs` (`LlmConfigPatch`)
- Modify: `crates/db/src/postgres/workspaces.rs` (`Workspace` struct, `WorkspacePatch` / `WorkspaceInsert` whatever the shapes are, SQL queries)
- Modify: `apps/api/src/domain/entity/workspace.rs` (likely a domain `Workspace` struct that mirrors the DB row)
- Modify: `apps/api/src/infrastructure/persistence/postgres/mapper.rs` (DB row → domain entity mapping)
- Modify: `apps/api/src/application/setup/defaults.rs` (any defaults that name `embedding_model`)

- [ ] **Step 1: Drop from `LlmConfigPatch`**

Remove `pub embedding_model: String,` from the patch struct in `apps/api/src/domain/port/workspace_repository.rs`. Update any constructor / `Default` impl.

- [ ] **Step 2: Drop from the `Workspace` struct(s)**

In `crates/db/src/postgres/workspaces.rs`: remove `pub embedding_model: String,` from the row struct (line 17 area) and from any `WorkspaceInsert` / `WorkspacePatch` analog (line 31, 104 area).

In `apps/api/src/domain/entity/workspace.rs`: remove the field from the domain `Workspace` entity if present.

- [ ] **Step 3: Update SQL queries in `workspaces.rs`**

The `INSERT INTO workspaces (...)` query at line 47 lists columns including `embedding_model`. Remove from the column list and the corresponding `.bind(...)` (line 58). The `UPDATE workspaces SET ...` query at line 121 references `embedding_model = $6` — remove that assignment and renumber subsequent placeholders. The `SELECT` queries that return `Workspace` rows must drop `embedding_model` from their `SELECT` column list.

- [ ] **Step 4: Update the mapper**

`apps/api/src/infrastructure/persistence/postgres/mapper.rs` may translate DB rows to domain entities and reference `embedding_model`. Drop.

- [ ] **Step 5: Update `defaults.rs`**

Any `embedding_model` constant or default in `apps/api/src/application/setup/defaults.rs` is removed.

- [ ] **Step 6: Update test doubles**

`apps/api/src/application/admin/test_doubles.rs` has `make_workspace` and `InMemoryWorkspaceRepository`. Both reference `embedding_model`. Drop the field from the fixture and any captured `LlmConfigPatch` shape.

- [ ] **Step 7: Verify**

```bash
cargo build --workspace
cargo test --workspace --lib --bins
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Expected: green. (The actual database still has the column at this point — the next task drops it. The struct-level change keeps the repo in sync with what the application talks about, and SQL `INSERT` with fewer columns is fine because the column has a default in the migration sense; if it doesn't, this task may leave the runtime DB in a state where INSERT fails at the column level. Confirm by running an integration test against a real DB OR proceed straight to Task 9 which drops the column.)

If `cargo build` fails because the SQL query no longer matches the table shape (sqlx may not statically check this in offline mode), this is acceptable — Task 9's migration brings them back in sync.

- [ ] **Step 8: Commit**

```bash
git add apps/api/ crates/db/
git commit -m "refactor(repo): drop embedding_model from Workspace + LlmConfigPatch + queries"
```

---

## Task 9: Migration — drop `workspaces.embedding_model`

**Files:**
- Create: `crates/db/migrations/0010_drop_embedding_model.sql`

- [ ] **Step 1: Write the migration**

Create `crates/db/migrations/0010_drop_embedding_model.sql`:

```sql
-- ============================================================
-- Drop the workspaces.embedding_model column.
--
-- Chronik handles embeddings server-side via its topic-level
-- vector.embedding.* configuration; the workspace-scoped column
-- is vestigial from the pre-Chronik (VexFS) era. The application
-- code stopped referencing it in the EmbeddingClient retirement
-- (see artifacts/plans/2026-05-01-embedding-client-retirement-plan.md).
-- ============================================================

ALTER TABLE workspaces
    DROP COLUMN embedding_model;
```

- [ ] **Step 2: Verify the migration applies**

```bash
docker compose up -d postgres
sqlx migrate run --source crates/db/migrations \
  --database-url "postgres://historiador_admin:devpassword@localhost:5432/historiador"
```

Expected: migration `0010` applies cleanly. If existing local data has `embedding_model NOT NULL`, the `DROP COLUMN` succeeds regardless.

(If the local DB has already been blown away or you're running against a fresh container, this is fine — `sqlx migrate run` runs all migrations from scratch.)

- [ ] **Step 3: Verify the API boots and tests pass against the migrated schema**

```bash
cargo run -p historiador_api --bin api &
sleep 5
curl -sS http://localhost:3001/health
# Expected: {"status":"ok"} or similar
kill %1
```

Then run the integration test if one exists:

```bash
cargo test --workspace
```

Expected: green, including any DB-touching integration tests.

- [ ] **Step 4: Commit**

```bash
git add crates/db/migrations/0010_drop_embedding_model.sql
git commit -m "feat(db): drop workspaces.embedding_model column (chronik owns embeddings)"
```

---

## Task 10: Drop `embedding_model` from the admin LLM settings form

**Files:**
- Modify: `apps/web/components/admin/llm-settings-form.tsx`

Re-read `apps/web/AGENTS.md` and `apps/web/CLAUDE.md` first.

- [ ] **Step 1: Drop the state + input**

Remove the `embeddingModel` state declaration (around line 51) and the `<Input label="Embedding model" ... />` field from the JSX (inside `renderApiKeyProviderForm` and `renderOllamaForm`, the `<div className="grid grid-cols-2 gap-4">` block — collapse the grid to a single-column layout or just one input).

- [ ] **Step 2: Drop from the submit payload**

The `await adminService.updateLlmConfig({ ... })` call drops `embedding_model: embeddingModel,`.

- [ ] **Step 3: Drop from the workspace prop sync**

The `useEffect` that syncs from `workspace` drops `setEmbeddingModel(workspace.embedding_model ?? "")` (or similar).

- [ ] **Step 4: Drop the requires-reindex banner**

Locate the JSX block that renders the "Changing the embedding model requires re-embedding X published page version(s)..." warning (likely conditional on `result.requires_reindex`). Remove the entire conditional block. Keep the manual "Re-index now" button if it's reachable through a different code path; if it's only inside the requires-reindex block, remove it (admins can still trigger reindex via a separate path — confirm by grepping for the reindex endpoint).

- [ ] **Step 5: Drop the `LlmPatchResponse` interface fields**

The TypeScript interface at line ~29 has `requires_reindex: boolean` and `affected_page_versions: number`. Remove. Update any reads of `result.requires_reindex` etc.

- [ ] **Step 6: Verify lint**

```bash
cd apps/web && pnpm lint
```

Expected: lint clean for `llm-settings-form.tsx`. Pre-existing unrelated errors (`editor/page.tsx:42`) remain out of scope.

- [ ] **Step 7: Commit**

```bash
git add apps/web/components/admin/llm-settings-form.tsx
git commit -m "feat(web): admin LLM settings drops embedding-model field"
```

---

## Task 11: Drop `embedding_model` from the setup wizard

**Files:**
- Modify: `apps/web/app/setup/page.tsx`

- [ ] **Step 1: Drop the state**

Remove `const [embeddingModel, setEmbeddingModel] = useState(...);`.

- [ ] **Step 2: Drop the input**

Remove the embedding-model `<Input>` JSX from the LLM step. Same for any Ollama-specific embedding-model `<Select>` (the wizard has a select for picking from `ollamaModels` for embedding — drop that select; the generation-model select stays).

- [ ] **Step 3: Drop from `canGoNext`**

The `case "llm":` arm references `embeddingModel.trim().length > 0`. Drop that conjunct. The Ollama branch's `embeddingModel` requirement also goes.

- [ ] **Step 4: Drop from the `/setup/init` payload**

The submit handler's body construction drops `embedding_model: embeddingModel || undefined,`.

- [ ] **Step 5: Verify lint**

```bash
cd apps/web && pnpm lint
```

Expected: lint clean for `setup/page.tsx`.

- [ ] **Step 6: Commit**

```bash
git add apps/web/app/setup/page.tsx
git commit -m "feat(web): setup wizard drops embedding-model field"
```

---

## Task 12: Update `LlmPatchBody` and frontend service types

**Files:**
- Modify: `apps/web/lib/services/admin.ts`

- [ ] **Step 1: Drop the field**

Remove `embedding_model: string;` from the `LlmPatchBody` interface (around line 49) and any `embedding_model` field from `LlmPatchResult`.

- [ ] **Step 2: Verify lint**

```bash
cd apps/web && pnpm lint
```

- [ ] **Step 3: Commit**

```bash
git add apps/web/lib/services/admin.ts
git commit -m "feat(web): LlmPatchBody drops embedding_model"
```

---

## Task 13: Regenerate OpenAPI + TypeScript types

**Files:**
- Modify: `openapi.yaml` (regenerated)
- Modify: `packages/types/generated/index.ts` (regenerated)

- [ ] **Step 1: Regenerate**

```bash
pnpm gen:types
```

- [ ] **Step 2: Inspect the diff**

```bash
git diff openapi.yaml packages/types/generated/index.ts | head -80
```

Expected: `embedding_model` removed from `LlmPatchRequest`, `SetupRequest`, `WorkspaceResponse`, `SetupResponse` (whichever DTOs carried it). `requires_reindex` and `affected_page_versions` removed from `LlmPatchResponse`. No other schema changes.

- [ ] **Step 3: Commit**

```bash
git add openapi.yaml packages/types/generated/index.ts
git commit -m "chore(types): regen openapi + ts types after embedding_model retirement"
```

---

## Task 14: Update env-example and docs

**Files:**
- Modify: `.env.example`
- Modify: `CLAUDE.md`
- Possibly: `docs/security.md`, `README.md`

- [ ] **Step 1: Trim `.env.example`**

Replace the `EMBEDDING_API_KEY` block (around lines 67-69) with a single comment clarifying that this env var is consumed by the **Chronik container** (not the app):

```bash
# EMBEDDING_API_KEY=sk-...
# Required for Chronik's built-in embedding pipeline. Forwarded to the
# Chronik container as both OPENAI_API_KEY and CHRONIK_EMBEDDING_API_KEY
# (see docker-compose.yml). Chronik embeds chunks server-side at index
# time and queries at search time; the app no longer constructs an
# embedding client.
```

- [ ] **Step 2: Update `CLAUDE.md`**

Search `CLAUDE.md` for "embedding_model" / "EmbeddingClient" / "embedding model". Update or remove references. Specifically the "Critical Invariants" section may mention embeddings; update to clarify Chronik owns embedding (and the workspace no longer carries the model name).

- [ ] **Step 3: Skim `docs/security.md` and `README.md` for stale references**

```bash
grep -n "embedding_model\|EmbeddingClient" docs/ README.md
```

If hits exist, edit minimally to remove the stale claim.

- [ ] **Step 4: Commit**

```bash
git add .env.example CLAUDE.md docs/ README.md
git commit -m "docs: clarify EMBEDDING_API_KEY is chronik-only after retirement"
```

(Run with whichever subset of paths actually changed.)

---

## Task 15: CHANGELOG entry

**Files:**
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Add a Removed/Changed entry under Unreleased**

```markdown
### Changed

- **BREAKING (API):** Removed `embedding_model` from `POST /setup/init`,
  `PATCH /admin/workspace/llm`, and `GET /admin/workspace`. Removed
  `requires_reindex` and `affected_page_versions` from
  `PATCH /admin/workspace/llm` response. Embeddings are owned by the
  Chronik vector store via topic-level configuration; the workspace
  no longer carries an embedding model. Existing clients that pass
  `embedding_model` will be rejected by validation; clients that read
  it from responses will see the field absent.

### Removed

- The `EmbeddingClient` trait and all its implementations
  (`OpenAiEmbeddingClient`, `OllamaEmbeddingClient`,
  `StubEmbeddingClient`) from the `historiador_llm` crate.
- The `embedding_client` field on `AppState`.
- The `workspaces.embedding_model` column (migration 0010).
- The "Changing the embedding model requires re-embedding ..." admin
  UX banner.
- The Anthropic-with-OpenAI-embeddings sidecar code path that read
  `EMBEDDING_API_KEY` to construct an OpenAI embedding client.
```

- [ ] **Step 2: Commit**

```bash
git add CHANGELOG.md
git commit -m "docs(changelog): note EmbeddingClient retirement and breaking API changes"
```

---

## Task 16: Final cargo + pnpm green check

- [ ] **Step 1: Run the full verification suite**

```bash
cargo build --workspace
cargo test --workspace --lib --bins
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
cd apps/web && pnpm lint
```

Expected: backend all green; frontend lint clean for the changed files (the pre-existing `editor/page.tsx:42` `react-hooks/set-state-in-effect` error remains and is unrelated).

- [ ] **Step 2: Boot the full stack and walk through setup**

```bash
docker compose down -v
docker compose up -d
cargo run -p historiador_api --bin api &
sleep 5
cd apps/web && pnpm dev &
sleep 10
```

Open `http://localhost:3000` and walk through the setup wizard with
`provider=test`. Confirm:
- The "Embedding model" field is absent.
- The wizard advances and completes.
- The admin LLM settings page (after login) does not show an embedding-model field.
- The "requires reindex" banner does not appear when changing the generation model.

Stop the dev servers when done.

- [ ] **Step 3: Final commit + open the PR (if combined branch route)**

If the branch route is the combined `feature/accept-url-openai`:

```bash
gh pr create --base develop --title "feat: openai custom base URL + retire EmbeddingClient" \
  --body "$(cat <<'EOF'
## Summary

This PR ships two related changes:

1. **OpenAI custom base URL** (per `artifacts/specs/2026-05-01-openai-custom-base-url-design.md`).
   Per-workspace `base_url` on the OpenAI provider, with optional API
   key for self-hosted no-auth endpoints (vLLM, LiteLLM, OpenRouter,
   LM Studio, Azure-via-proxy).

2. **EmbeddingClient retirement** (per
   `artifacts/plans/2026-05-01-embedding-client-retirement-plan.md`).
   The chunk pipeline and MCP search no longer construct an embedding
   client; Chronik owns embedding via topic-level configuration. Drops
   the trait, its impls, the `embedding_client` AppState field, the
   `workspaces.embedding_model` column, and the "requires reindex" UX.

## Breaking changes (API)

- `POST /setup/init` no longer accepts `embedding_model`.
- `PATCH /admin/workspace/llm` no longer accepts `embedding_model`.
  Response no longer carries `requires_reindex` or
  `affected_page_versions`.
- `GET /admin/workspace` no longer returns `embedding_model`.

## Test plan

- [ ] `cargo test --workspace` green
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` green
- [ ] `cd apps/web && pnpm lint` green for changed files
- [ ] Migration 0010 applies cleanly against an existing dev DB
- [ ] Manual setup-wizard walk-through with `provider=test`
- [ ] Manual admin-LLM-config walk-through against a local LiteLLM mock
- [ ] Verify a published page's chunks land in Chronik (Chronik handles embedding server-side)

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

If the branch route is a fresh branch off develop after the base-URL PR merges, replace `feature/accept-url-openai` with `feature/retire-embedding-client` and adjust the PR title/body to scope only to the retirement.

---

## Self-review

After all 16 tasks land, check:

- **Coverage:** every spec/decision (D1-D8) maps to at least one task.
- **Type consistency:** `LlmConfigPatch`, `Workspace`, `LlmPatchRequest`, `SetupRequest`, `LlmPatchBody` all consistently lack `embedding_model`. `UpdateLlmConfigResult` consistently lacks `requires_reindex` / `affected_page_versions`.
- **Dead code:** no remaining hits for `EmbeddingClient` / `OpenAiEmbeddingClient` / `OllamaEmbeddingClient` / `StubEmbeddingClient` / `embedding_client:` outside the deleted-already-gone sense:
  ```bash
  grep -rn "EmbeddingClient\|embedding_client\|embedding_model" apps/ crates/ packages/ 2>/dev/null | grep -v "^Binary\|/target/\|/.next/\|/node_modules/"
  ```
  Expected: zero hits in source files.
- **Probe:** the OpenAI probe arm hits `GET /models` against the supplied URL and never names a specific model.
- **Migration sequence:** `0001_initial_schema.sql` → ... → `0010_drop_embedding_model.sql`. No gaps.
- **OpenAPI schema:** `LlmPatchRequest`, `SetupRequest`, `WorkspaceResponse` no longer mention `embedding_model`. `LlmPatchResponse` no longer mentions `requires_reindex` or `affected_page_versions`.
- **CHANGELOG:** breaking API changes are flagged.
