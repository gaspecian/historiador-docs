# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

Historiador Doc is a self-hosted documentation platform where every knowledge base ships with a built-in MCP server. Two things make it unusual:

1. **Dual representation** — pages are authored as human-readable markdown *and* stored as structure-aware chunks in a vector store (VexFS). Authors never see chunks; AI tools never see raw markdown. The chunker bridges the two.
2. **MCP-native** — the MCP endpoint is a standalone, read-only Axum service (not a plugin). Companies expose only the MCP port externally while keeping the authoring API internal.

## Architecture

Mixed-language monorepo: Rust backend (Cargo workspace) + Next.js frontend (pnpm workspace), orchestrated by Turborepo.

```
apps/
  api/          Axum REST API         (port 3001, internal)
  mcp/          Axum MCP server       (port 3002, externally exposed, read-only)
  web/          Next.js 16 + React 19 (port 3000, Tailwind 4)
crates/
  db/           Shared Postgres (sqlx) + Chronik-Stream clients; owns migrations
  chunker/      Structure-aware markdown chunker (comrak AST)
  llm/          EmbeddingClient + TextGenerationClient traits; OpenAI, Anthropic, Ollama, stub impls
packages/
  types/        TypeScript types auto-generated from openapi.yaml
```

**Rust package names use underscores**: `historiador_api`, `historiador_db`, `historiador_llm`, `historiador_mcp`, `historiador_chunker`.

The API binary (`apps/api`) is the single entry point for migrations, auth, CRUD, and the AI editor. AppState (`apps/api/src/state.rs`) holds the PgPool, JWT secret, cipher, LLM clients, and vector store — injected as `State<Arc<AppState>>` in all handlers. Routes are composed in `apps/api/src/routes.rs` and mounted by `apps/api/src/app.rs`.

The frontend (`apps/web`) proxies `/api/*` requests to the Axum API via a Next.js rewrite in `next.config.ts` (target: `API_INTERNAL_URL`, defaults to `http://localhost:3001`). All API calls go through `apps/web/lib/api.ts` (`apiFetch`), which handles JWT injection, 401 refresh, and 423 setup-gate redirect.

## Critical Invariants

Violating any of these breaks the architecture. Read the linked ADR before proposing changes:

- **MCP server has zero write access.** The Postgres role `historiador_mcp` has SELECT-only grants on a whitelisted subset of tables. The MCP binary uses `DATABASE_URL_READONLY`. Docker/env config must never leak write credentials to MCP. See [ADR-003](artifacts/adr/ADR-003-mcp-server-architecture.md).
- **Chronik-Stream is the retrieval source of truth; PostgreSQL is the content/metadata source of truth.** Chunk embeddings live in Chronik-Stream; page markdown, users, collections, and language config live in PostgreSQL. Do not duplicate. [ADR-007](artifacts/adr/ADR-007-chronik-stream.md) supersedes [ADR-001](artifacts/adr/ADR-001-vector-database.md) — Chronik replaced VexFS and is now implemented.
- **Chunks are structure-aware, never fixed-size.** The chunker walks the markdown AST at heading boundaries (H1→H2→H3) and never splits mid-section. Code blocks, tables, and lists are atomic. See [ADR-002](artifacts/adr/ADR-002-chunking-strategy.md).
- **Every chunk carries a `language` field (BCP 47).** Language is a workspace-level setting configured at installation. The `page_versions` table is keyed by `(page_id, language)`. See [ADR-005](artifacts/adr/ADR-005-multilingual-architecture.md).
- **OpenAPI is the single source of truth for the API contract.** `apps/api` uses `utoipa` annotations to emit `openapi.yaml` at build time; `openapi-typescript` generates `packages/types/generated/`. Never hand-edit generated types. Never add an API route without a `#[utoipa::path]` annotation.
- **ADR-006 supersedes ADR-004** (Rust backend was chosen over Node.js). Read the relevant ADR before making architectural suggestions — these decisions are settled unless the user explicitly reopens them.

## AI Editor (Sprint 11 — in progress)

Sprints 01–10 are shipped. Sprint 11 (current branch `feature/sprint-11`) is designing/building the AI editor across ADRs 008–016. Before touching editor transport, canvas state, LLM tool calls, or proposal/comment flows, read the relevant ADR:

- [ADR-008](artifacts/adr/ADR-008-split-pane-editor.md) — split-pane editor layout
- [ADR-009](artifacts/adr/ADR-009-websocket-transport-reaffirm.md) — WebSocket transport (reaffirms the move off SSE from v1.0)
- [ADR-010](artifacts/adr/ADR-010-canvas-block-tree.md) — canvas block tree model
- [ADR-011](artifacts/adr/ADR-011-llm-tool-calling.md) — LLM tool calling contract
- [ADR-012](artifacts/adr/ADR-012-editor-message-envelope.md) — editor message envelope
- [ADR-013](artifacts/adr/ADR-013-proposal-overlay.md) — proposal overlay
- [ADR-014](artifacts/adr/ADR-014-autonomy-modes.md) — autonomy modes
- [ADR-015](artifacts/adr/ADR-015-outline-event.md) — outline event
- [ADR-016](artifacts/adr/ADR-016-inline-comments.md) — inline comments

## Development Commands

### Prerequisites

- Docker (Compose v2) — runs Postgres + Ollama
- Rust stable toolchain (via `rust-toolchain.toml`: stable + rustfmt + clippy)
- Node.js 20+ with pnpm (`corepack enable`)

### Start the stack

```bash
cp .env.example .env              # first time only; never commit .env
docker compose up -d              # Postgres (5432), Ollama (11434), Chronik (9092 Kafka / 6092 SQL)
cargo run -p historiador_api --bin api   # API on :3001 (runs migrations on boot)
# In another terminal:
cd apps/web && pnpm dev           # Next.js on :3000
```

### First-run setup

Until the setup wizard completes, all API endpoints (except `/health`, `/setup/init`, `/setup/probe`, `/docs/`) return `423 Locked`. Open `http://localhost:3000` to run the wizard, or POST to `/setup/init` directly (see README for curl examples).

Supported `llm_provider` values: `"ollama"`, `"openai"`, `"anthropic"`, and `"test"`. The `"test"` provider skips all LLM validation — use it to complete setup in E2E tests or when working offline without configuring a real model.

### Rust

```bash
cargo build --workspace                                              # compile
cargo test --workspace                                               # test
cargo clippy --workspace --all-targets --all-features -- -D warnings # lint (CI denies warnings)
cargo fmt --all --check                                              # format check (CI requires)
```

Run a single test: `cargo test -p historiador_db test_name`

### TypeScript / Frontend

```bash
pnpm install                     # root workspace
cd apps/web && pnpm dev          # Next.js dev server
cd apps/web && pnpm lint         # ESLint (eslint-config-next)
```

### OpenAPI codegen pipeline

After changing any `#[utoipa::path]` or `ToSchema` annotation in Rust:

```bash
pnpm gen:types    # runs gen:openapi → build:types
```

Both `openapi.yaml` and `packages/types/generated/index.ts` are committed so contributors can read the API contract without a full build.

### Turbo task graph

`build:rust` → `gen:openapi` → `build:types` → `build` (Next.js). Run the full pipeline with `turbo build`.

### Database

Migrations live in `crates/db/migrations/` and are embedded in the API binary via `sqlx::migrate!`. They run automatically on API boot. To run manually:

```bash
sqlx migrate run --source crates/db/migrations \
  --database-url "postgres://historiador_admin:devpassword@localhost:5432/historiador"
```

Two Postgres roles enforce ADR-003 at the DB layer:
- `historiador_api` — owns tables, full CRUD (created by `docker/postgres/init/10-roles.sh`)
- `historiador_mcp` — SELECT only on `workspaces, collections, pages, page_versions, chunks`

### CI

Three parallel jobs (`.github/workflows/ci.yml`):
1. **Rust**: fmt → clippy (deny warnings) → test → release build
2. **Node**: pnpm install → lint
3. **Docker**: smoke-build api, mcp, and web images (no push)

### Docker builds

`Dockerfile.rust` uses cargo-chef for dependency layer caching. Select binary with `--build-arg BIN_NAME=api|mcp`.

## MCP protocol surface

`POST /mcp` speaks [Model Context Protocol](https://modelcontextprotocol.io/) over JSON-RPC 2.0 (protocol version `2025-03-26`): `initialize`, `tools/list`, `tools/call`. It exposes a single `query` tool whose `inputSchema` accepts `query` (required), `language` (BCP 47, optional), and `top_k` (1–20, default 5). Auth is `Authorization: Bearer <token>` with constant-time SHA-256 comparison against the stored digest. `POST /query` is an internal REST alias used by the web UI and is *not* part of the MCP public contract.

## Reference docs

- [docs/security.md](docs/security.md) — security posture, dependency audit, constant-time MCP token comparison, Postgres role separation.
- [docs/performance.md](docs/performance.md) — p95 < 2 s target for 1,000 queries over 10,000 chunks; load-test script at `scripts/load-test/run.sh`.
- [CONTRIBUTING.md](CONTRIBUTING.md) — local setup, OpenAPI → TypeScript pipeline, PR conventions.
- [CHANGELOG.md](CHANGELOG.md) — canonical "Known Limitations" list for v1.0.

## Conventions

### Artifacts

- **ADRs are append-only.** To change a decision, write a new ADR that supersedes the old one (as ADR-006 did to ADR-004). Do not edit accepted ADRs retroactively.
- Sprint files in `artifacts/sprints/` are historical snapshots — don't rewrite them.
- The PRD's "Resolved Decisions" table is load-bearing — if a decision changes, update both the table and any affected ADR.

### Code organization

- **Binaries** in `apps/` (api, mcp, web). **Libraries** in `crates/` (db, chunker, llm).
- Frontend components: `apps/web/components/` (UI primitives in `ui/`, feature-specific in named dirs).
- Frontend hooks/utils: `apps/web/lib/`.
- The `apps/web/AGENTS.md` warns that this is Next.js 16 with breaking changes from earlier versions — read `node_modules/next/dist/docs/` before writing Next.js code.

### Environment

Two distinct DB connection strings prevent accidental credential reuse:
- `DATABASE_URL_READWRITE` — used by the API binary
- `DATABASE_URL_READONLY` — used by the MCP binary

Auth secrets (`JWT_SECRET`, `APP_ENCRYPTION_KEY`) must be at least 32 chars/bytes. The API crashes at boot if they're missing or too short.
