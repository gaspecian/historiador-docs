# Custom Base URL for the OpenAI Provider

**Date:** 2026-05-01
**Branch:** `feature/accept-url-openai`
**Status:** Draft (awaiting user review)

## 1. Goal

Allow workspace admins to point the `openai` LLM provider at any
OpenAI-compatible HTTP endpoint (Azure-via-proxy, OpenRouter,
LiteLLM, vLLM, LM Studio, Together, Fireworks, etc.) and to optionally
omit the API key for self-hosted servers that don't require auth.

When no custom base URL is set, behavior is unchanged: the provider
hits `https://api.openai.com/v1` with an `Authorization: Bearer <key>`
header.

## 2. Non-goals

- Native Azure OpenAI auth shape (`api-key` header,
  `/openai/deployments/{deployment}/...` URL pattern, `api-version`
  query). Azure users are expected to front Azure with a LiteLLM (or
  similar) proxy that exposes a standard OpenAI surface.
- Separate base URLs for embeddings vs. chat. One URL serves both.
- Process-wide env-var override of the workspace setting. URL lives
  per-workspace in PostgreSQL, edited via the same surfaces (setup
  wizard + admin LLM config form) that already manage the API key.
- Multiple base URLs per workspace, or per-page / per-collection
  overrides.
- Allowlist of permitted hosts. This is a self-hosted product;
  admins are trusted to point at the right endpoint.

## 3. Configuration semantics

`workspaces.llm_base_url` (`TEXT`, nullable) already exists from the
Ollama work — no migration is needed. Its semantics are extended
for the `openai` provider:

| `llm_provider` | `llm_base_url` | `llm_api_key_encrypted` | Behavior |
|---|---|---|---|
| `openai` | `NULL` | required | Hits `https://api.openai.com/v1` with bearer auth (unchanged from today). |
| `openai` | set    | required | Hits the custom base URL with bearer auth. |
| `openai` | set    | `NULL`   | Hits the custom base URL with **no** `Authorization` header. |
| `openai` | `NULL` | `NULL`   | Rejected at validation (unchanged). |
| `ollama` | required | `NULL` | Unchanged. URL still arrives via the `llm_api_key` form field for backwards compatibility. |
| `anthropic` | `NULL` | required | Unchanged. |
| `test` | `NULL` | `NULL` | Unchanged. |

The Ollama-specific overload of the `llm_api_key` form field (which
carries the URL today) is **not** changed by this work. Only the
OpenAI provider gains a separate `base_url` form field.

## 4. URL validation rules

A submitted base URL must:

1. Parse as a valid absolute URL (`url::Url::parse`).
2. Use scheme `http` or `https`. (`http` is allowed because the
   product is self-hosted and admins routinely run local LLM servers
   on plain HTTP.)
3. Carry no userinfo component. URLs of the shape
   `https://user:pass@host/v1` are rejected — auth must come from
   the bearer field.
4. Have its trailing slash normalized off before persistence.

The model path suffix (`/v1`, `/v1/`, or no suffix) is **not**
mandated; it depends on the target server's routing. `async-openai`
appends the operation path to whatever base URL it receives.

## 5. Components changed

### 5.1 `crates/llm/src/openai.rs`

Both `OpenAiEmbeddingClient` and `OpenAiTextGenerationClient` gain a
new constructor that accepts an optional base URL and an optional
API key:

```rust
pub struct OpenAiEmbeddingConfig<'a> {
    pub api_key:  Option<&'a str>,
    pub base_url: Option<&'a str>,
    pub model:    &'a str,
    pub dim:      usize,
}

pub struct OpenAiGenerationConfig<'a> {
    pub api_key:  Option<&'a str>,
    pub base_url: Option<&'a str>,
    pub model:    &'a str,
}

impl OpenAiEmbeddingClient {
    pub fn from_config(cfg: OpenAiEmbeddingConfig<'_>) -> Self { ... }
}
impl OpenAiTextGenerationClient {
    pub fn from_config(cfg: OpenAiGenerationConfig<'_>) -> Self { ... }
}
```

Existing `new(api_key)` and `with_model(...)` constructors stay as
thin wrappers calling `from_config` so unrelated call sites and tests
keep compiling.

When `base_url` is `Some`, the client builds
`OpenAIConfig::new().with_api_base(url).with_api_key(key)`. When
`api_key` is `None`, the client must avoid sending the
`Authorization` header. `async-openai` v0.x sends `Bearer <key>`
unconditionally — including `Bearer ` (empty) when the key is empty.
Implementation note: pass an explicit `reqwest::Client` to
`Client::with_http_client(...)` and rely on `OpenAIConfig` with no
key set, OR (verified during implementation) pass an empty key and
strip the header via a `reqwest` middleware. The exact mechanism is
chosen in the plan after a 30-line spike against `async-openai`'s
current behavior.

### 5.2 Probe — `apps/api/src/infrastructure/llm/probe.rs`

The `LlmProbe` trait method takes the new optional URL:

```rust
#[async_trait]
pub trait LlmProbe: Send + Sync + 'static {
    async fn probe(
        &self,
        provider: LlmProvider,
        api_key: &str,
        base_url: Option<&str>,
    ) -> anyhow::Result<()>;
}
```

The OpenAI probe builds an `OpenAiEmbeddingClient` from
`from_config`, then runs a single embedding call against the
workspace's chosen embedding model. Failures (transport, TLS, auth,
4xx, 5xx) are bubbled up so the use case can surface them via
`Validation("LLM rejected: ...")`.

Existing call sites (`update_llm_config.rs`, the standalone
`/setup/probe` handler in `apps/api/src/presentation/handler/setup.rs`)
are updated to pass the new argument.

### 5.3 Application use cases

- `UpdateLlmConfigCommand` (in
  `apps/api/src/application/admin/update_llm_config.rs`) gains
  `base_url: Option<String>`.
- `InitializeInstallationCommand` (in
  `apps/api/src/application/setup/initialize_installation.rs`) gains
  `base_url: Option<String>`.

In both, the OpenAI branch persists the new value verbatim (after
URL normalization). Validation rules:

- Reject when `provider == openai && base_url.is_none() && api_key.is_empty()`.
- When `provider == openai && base_url.is_some()`, an empty
  `api_key` is allowed and persisted as `NULL`.
- When `provider == openai && base_url.is_none()`, an empty
  `api_key` keeps the existing semantics ("don't rotate the secret").

The Ollama branch is untouched; it continues to read the URL from
`llm_api_key` for backwards compatibility.

### 5.4 HTTP DTOs (presentation layer)

- `LlmPatchRequest`
  (`apps/api/src/presentation/handler/admin/workspace.rs`) gains
  an optional `base_url: Option<String>` field with
  `#[validate(length(max = 512), custom(function = "validate_llm_base_url"))]`.
  The `validator` crate's built-in `url` rule only checks parse
  success — the §4 rules (scheme `http`/`https`, no userinfo,
  trailing-slash normalization) require a custom validator that
  lives next to the DTO.
- `SetupRequest`
  (`apps/api/src/presentation/handler/setup.rs`) gains the same
  optional field.
- The `/setup/probe` request DTO (`SetupProbeRequest` in the same
  file) gains the same optional field so admins can validate a URL
  before committing.

After these DTO changes, run `pnpm gen:types` to regenerate
`openapi.yaml` and `packages/types/generated/index.ts`.

### 5.5 Boot-time client construction — `apps/api/src/main.rs`

`build_llm_clients_from_workspace` already reads `ws.llm_base_url`
for Ollama; the `openai` and `anthropic`-with-OpenAI-embeddings
arms are updated to pass the workspace's `llm_base_url.as_deref()`
into `from_config`, and to pass `Some(key.as_str())` when the
decrypted key is non-empty or `None` otherwise.

The pre-setup env-var fallback (`LLM_PROVIDER` / `LLM_API_KEY`) is
**not** extended in this work — see non-goal #3. Pre-setup, OpenAI
continues to hit `api.openai.com` only.

### 5.6 Frontend — `apps/web`

In the setup wizard
(`apps/web/app/setup/...`) and the admin LLM config form (whichever
page hosts `LlmPatchRequest`):

- Add a "Base URL (optional)" text field, only shown for
  `provider === "openai"`.
- When the field is filled, the API key field's required indicator
  is removed and an "(optional for self-hosted)" hint appears.
- Client-side validation: must parse as URL with scheme
  `http`/`https`, no userinfo, length ≤ 512.
- Same field appears alongside the "Test connection" button on
  `/setup/probe` so admins can verify reachability before submitting.

The TypeScript types come from regenerated `packages/types/generated`,
so the request bodies pick up `base_url` automatically.

## 6. Data flow

```
Admin form
  └─ POST /admin/workspace/llm
       { llm_provider: "openai",
         llm_api_key?: "...",
         base_url?:  "https://my-litellm.example/v1",
         generation_model, embedding_model }
       │
       ▼
   LlmPatchRequest → UpdateLlmConfigCommand
       │
       ▼
   UpdateLlmConfigUseCase
       ├─ probe.probe(OpenAi, key, Some(base_url))      ← if rotating key
       ├─ encrypt(key) if non-empty                     ← Cipher
       └─ workspaces.update_llm_config(..., llm_base_url = base_url, ...)
       │
       ▼
   { requires_reindex, requires_restart }

Boot / next request
   build_llm_clients_from_workspace(cipher, ws)
       └─ OpenAiEmbeddingClient::from_config({ api_key, base_url, model, dim })
       └─ OpenAiTextGenerationClient::from_config({ api_key, base_url, model })
       │
       ▼
   async-openai Client → POST {base_url}/embeddings, /chat/completions
```

## 7. Error handling

- **Probe transport error** (DNS, connection refused, TLS):
  `Validation("LLM rejected: <preserved underlying error>")`. The
  message must be specific enough that an admin can tell "wrong
  URL" from "wrong key".
- **Probe HTTP 4xx/5xx**: same envelope, including the response
  status and any text body trimmed to a sensible length (≤ 512
  chars) to avoid leaking large HTML error pages into the UI.
- **Invalid URL at validation**: rejected at the DTO layer
  (`validator` crate), client never reaches the use case.
- **`provider == openai`, no URL, no key**: same `Validation` error
  as today's "OpenAI requires a key" path.
- **URL with embedded credentials**: rejected at validation with a
  message pointing the admin at the bearer field.
- **Decryption failure** at boot: existing behavior; the API
  fails fast with the existing `cipher.decrypt(...)` error.

## 8. Testing

### 8.1 `crates/llm/src/openai.rs` (unit)

- Construct embedding + text-gen clients with each combination of
  (api_key: present/absent) × (base_url: present/absent). Verify
  the resulting `OpenAIConfig` carries the expected fields.
- Spin up a `wiremock` server that asserts:
  - With `api_key=Some` → request has `Authorization: Bearer <key>`.
  - With `api_key=None` → request has **no** `Authorization`
    header.
  - The path is `{base_url_path}/embeddings` /
    `{base_url_path}/chat/completions`.

### 8.2 Use cases (unit)

- `UpdateLlmConfigUseCase` validation matrix from §3 (rejects
  invalid combos, accepts the new ones).
- `InitializeInstallationUseCase` validation matrix from §3.
- Confirm `base_url` is persisted for `openai` and `ollama` only;
  `anthropic` and `test` always persist `NULL`.

### 8.3 Probe (unit)

- `LlmProbe::probe(OpenAi, key, Some(url))` with a `wiremock` server
  that returns `200 { data: [{embedding: [...]}, ...] }` → success.
- Same with `401` → bubbled-up error contains "401".
- Same with connection-refused → bubbled-up error mentions transport.

### 8.4 HTTP layer (integration)

- `PATCH /admin/workspace/llm` with `base_url` set and empty key:
  succeeds for an authenticated admin, persists `base_url`, leaves
  `llm_api_key_encrypted` `NULL`.
- `POST /setup/init` with `base_url` and empty key: same.
- `POST /setup/probe` with `base_url`: probes and returns
  success/failure shape.

### 8.5 OpenAPI

- After DTO changes, `pnpm gen:types` produces no diff beyond the
  new `base_url` field in the three relevant request schemas.

### 8.6 Frontend (component / E2E)

- Setup wizard: choosing `openai` reveals the URL field; filling it
  removes the API-key required indicator. Submit succeeds.
- Admin LLM config: same for the patch form.

## 9. Migration & rollout

- **No DB migration.** `workspaces.llm_base_url` already exists.
- **No breaking change** to the public API. The new `base_url`
  field is optional; existing clients keep working.
- **Default behavior unchanged** for existing workspaces: their
  `llm_base_url` stays `NULL` and OpenAI continues to hit
  `api.openai.com`.
- **CHANGELOG entry**: under the next release's "Added" section,
  one line: "OpenAI provider now accepts an optional custom base
  URL and an optional API key for self-hosted OpenAI-compatible
  endpoints (vLLM, LiteLLM, OpenRouter, etc.)."

## 10. Open implementation question (decided in plan)

How exactly to suppress the `Authorization` header when `api_key`
is `None`. `async-openai`'s public `OpenAIConfig` does not expose a
"no auth" mode; we must either:

1. Provide our own `reqwest::Client` to `Client::with_http_client`
   and add a middleware that strips `Authorization` when empty.
2. Use a thin custom `Config` impl from `async-openai`'s `Config`
   trait that returns the right headers conditionally.

This is a 30-line spike during the first plan task; the design
allows either choice without changing the rest of the system.
