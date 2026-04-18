# Changelog

All notable changes to Historiador Doc are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.0] — 2026-05-01

First public release. Self-hosted documentation platform with a built-in
MCP server — authors write markdown, AI agents query structure-aware
chunks.

### Added

- **MCP-native retrieval.** JSON-RPC 2.0 endpoint at `POST /mcp` speaks
  Model Context Protocol version `2025-03-26`, implementing
  `initialize`, `tools/list`, and `tools/call`. Claude Desktop and
  other MCP clients can discover and invoke the `query` tool directly.
- **Dual representation.** Pages are authored as markdown (stored in
  Postgres) and indexed as structure-aware chunks (stored in
  Chronik-Stream). Chunks never split mid-section; heading boundaries
  are atomic (ADR-002).
- **Multilingual by default.** Every chunk carries a BCP 47 `language`
  tag. `page_versions` is keyed by `(page_id, language)`. Workspaces
  declare their language set at setup (ADR-005).
- **Split-pane AI editor** (SSE transport for v1.0; ADR-009):
  conversation-style authoring alongside a live markdown preview,
  with **debounced auto-save** (2 s idle, 30 s max) and **persisted
  conversation history** keyed by `(page_id, language, user_id)`.
- **Page version history** with list / view / restore-as-draft
  (Sprint 7).
- **Full-workspace markdown export** as a ZIP archive.
- **First-run setup wizard** behind a `423 Locked` gate that blocks
  every endpoint except `/health`, `/setup/*`, and `/docs/`.
- **RBAC** with Admin, Author, and Viewer roles, enforced in the
  application layer (route-level middleware tracked for v1.1).
- **LLM providers**: Ollama (local), OpenAI, Anthropic. LLM API keys
  encrypted at rest with AES-GCM (key = `APP_ENCRYPTION_KEY`).
- **Analytics dashboard** with MCP query tracking, zero-result
  detection, and frequency analysis powered by Chronik's SQL/Arrow
  layer.
- **Setup-gate middleware** and **OpenAPI → TypeScript codegen
  pipeline** as the single source of truth for the HTTP contract.
- **Production compose profile** (`docker-compose.prod.yml`), a sample
  reverse-proxy config (`docs/deploy/nginx.conf`), and a load-test
  harness (`scripts/load-test/run.sh` + `load-test-seed` binary).

### Security

- **Argon2id** password hashing with the `argon2` crate defaults.
- **JWT HS256** with mandatory ≥32-char secret; API crashes at boot
  if `JWT_SECRET` is missing or too short.
- **Timing-safe MCP bearer token validation** via
  `subtle::ConstantTimeEq` over a SHA-256 digest; regression test
  asserts the comparison does not short-circuit on a prefix match.
- **Postgres role separation** (ADR-003):
  - `historiador_api` — read/write on app tables.
  - `historiador_mcp` — SELECT only on `workspaces`, `collections`,
    `pages`, `page_versions`, `chunks`.
- **Durable-only vector store by default.** The in-memory fallback is
  gated behind `ALLOW_IN_MEMORY_VECTOR_STORE`; production deploys
  refuse to boot if Chronik is unreachable.
- **`cargo audit`** clean at release (RUSTSEC-2026-0098 / 0099 on
  `rustls-webpki` patched via the 0.103.12 bump).
- Complete security posture documented in [`docs/security.md`](docs/security.md).

### Changed

- The MCP endpoint moved from a custom `POST /query` JSON shape to
  JSON-RPC 2.0 at `POST /mcp`. The legacy `/query` is retained as an
  internal alias for the web UI. Claude Desktop configuration should
  now point at `/mcp`.
- README's "Known Limitations" section no longer claims missing version
  history or VexFS-only vector storage; both were superseded in
  Sprints 7 and 7 respectively.

### Known Limitations

- **Editor transport is Server-Sent Events**, not the WebSocket model
  originally envisioned by ADR-008. ADR-009 ratifies SSE for v1.0; the
  WebSocket rebuild with an explicit conversation/generation mode
  toggle and inline section click-to-edit is tracked for v1.1.
- **Route-level RBAC middleware** is not present. Authorization is
  enforced in the use-case layer today; a defence-in-depth middleware
  layer is tracked for v1.1.
- **Invitation emails** must be shared manually — the invitation API
  returns an activation URL string; native email delivery ships in v1.1.
- **Native Ollama embeddings** are stubbed. If you configure Ollama as
  the LLM provider, embeddings fall back to the stub embedder.
  OpenAI / Anthropic embeddings work natively.
- **Chunker metadata** does not yet include `author_id` or
  `last_updated` on the emitted chunks; the source `page_versions` row
  carries both, but they are not propagated into the vector payload.

### What's Next (v2)

- **Gap-detection flywheel.** MCP queries that return zero results
  cluster into documentation requests, surfaced in the admin dashboard.
- **SSO/SAML** (Okta, Azure AD, Google Workspace).
- **Multi-workspace** support on a single deploy.
- **Public documentation site** with custom domains and a read-only
  public MCP endpoint per site.
- **Webhooks** for publish / chunk-indexed / query-received events.

See [`artifacts/product/historiador-doc-prd-v2.md`](artifacts/product/historiador-doc-prd-v2.md)
for the full v2 direction.
