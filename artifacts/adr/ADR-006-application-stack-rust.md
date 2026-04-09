# ADR-006: Application Stack — Next.js Frontend + Rust Backend

**Status:** Accepted
**Date:** 2026-04-08
**Supersedes:** [ADR-004](ADR-004-application-stack.md)
**Deciders:** Gabriel Specian (Nexian Tech)

---

## Context

ADR-004 proposed a full Node.js/TypeScript stack (Fastify for the API and MCP server). This decision is superseded by the choice to use **Rust for all backend services**, with Next.js retained for the frontend.

The primary drivers for Rust:
- The MCP server has a strict < 2 second p95 latency target — Rust's predictable, GC-free performance makes this easier to guarantee
- The chunking pipeline is CPU-intensive (markdown AST traversal, tokenization, embedding calls) — Rust's native concurrency model handles this without the event loop constraints of Node.js
- VexFS is the vector database underpinning the system; having backend services in Rust creates natural alignment if VexFS is Rust-native or has a first-class Rust client
- Rust's memory safety guarantees reduce entire classes of runtime bugs without sacrificing performance

The monorepo uses **pnpm workspaces** for the JavaScript/TypeScript layer (Next.js frontend, shared type packages) and a **Cargo workspace** for the Rust layer, orchestrated together by **Turborepo**.

---

## Decision

**Use a mixed-language monorepo:**
- **Frontend**: Next.js (React, TypeScript) — dashboard, AI editor, admin panel, setup wizard
- **Backend API**: Rust with Axum — page management, user management, chunking pipeline, language validation
- **MCP Server**: Rust with Axum — standalone service, read-only, implements MCP protocol
- **Monorepo tooling**: pnpm workspaces (TypeScript layer) + Cargo workspace (Rust layer), orchestrated by Turborepo
- **API contract**: OpenAPI schema generated from Rust (via `utoipa`) → TypeScript types generated for the frontend (via `openapi-typescript`)

---

## Options Considered

### Option A: Next.js + Rust Backend ✅ Selected

| Dimension | Assessment |
|-----------|------------|
| MCP latency target | Excellent — compiled, no GC pauses |
| CPU-intensive tasks | Excellent — native threads, no event loop |
| LLM SDK support | Good — `async-openai` is mature; Anthropic via HTTP |
| Markdown AST parsing | Good — `comrak` provides CommonMark AST |
| Type sharing (frontend ↔ backend) | Via OpenAPI codegen — one extra build step |
| Open-source contribution barrier | Medium-High — Rust has a steeper learning curve |
| Docker footprint | Excellent — Rust binaries are small and statically linkable |

**Pros:**
- Predictable, low-latency performance for the MCP server — no garbage collection pauses, no event loop saturation
- Rust's ownership model makes the chunking pipeline memory-safe without manual management
- Rust async (Tokio) handles high concurrency efficiently — embedding generation and MCP queries can be parallelized cleanly
- Small, statically-linked Docker images — the Rust services compile to single binaries with minimal runtime dependencies
- Natural alignment with VexFS if its client library is Rust-native
- Axum (from the Tokio team) is the most modern, actively maintained Rust web framework — excellent ecosystem (tower middleware, tracing, sqlx)

**Cons:**
- Mixed-language monorepo requires careful tooling setup — Turborepo must orchestrate both `cargo build` and `pnpm build` pipelines
- Type safety across the frontend/backend boundary requires OpenAPI codegen — not as seamless as a single TypeScript monorepo
- Rust's compile times are longer than TypeScript — local development feedback loops are slower
- Higher open-source contributor bar compared to a full TypeScript stack

---

### Option B: Full Node.js / TypeScript Stack (ADR-004, superseded)

| Dimension | Assessment |
|-----------|------------|
| MCP latency target | Good — achievable but requires careful async hygiene |
| CPU-intensive tasks | Requires worker threads to avoid event loop blocking |
| Type sharing | Excellent — native TypeScript monorepo |
| Contribution barrier | Low |
| Docker footprint | Medium |

Superseded — see ADR-004 for full analysis. The decision to use Rust was made after evaluating the performance requirements of the MCP server and the CPU intensity of the chunking pipeline.

---

### Option C: Python Backend (FastAPI) + Next.js Frontend

Already evaluated and rejected in ADR-004. Two-language penalty, larger Docker footprint, and no shared-type benefit not justified by Python's AI ecosystem advantage for Historiador Doc's specific use case.

---

## Monorepo Structure

```
historiador-doc/
├── apps/
│   └── web/                        # Next.js frontend (TypeScript)
│       ├── src/
│       └── package.json
├── packages/
│   └── types/                      # Auto-generated TypeScript types from OpenAPI
│       ├── generated/              # Output of openapi-typescript (do not edit)
│       └── package.json
├── crates/                         # Rust Cargo workspace
│   ├── api/                        # Axum API server (port 3001)
│   │   └── src/
│   ├── mcp/                        # Axum MCP server (port 3002)
│   │   └── src/
│   ├── chunker/                    # Structure-aware markdown chunker library
│   │   └── src/
│   └── db/                         # Shared VexFS + PostgreSQL clients (Rust)
│       └── src/
├── Cargo.toml                      # Rust workspace root
├── pnpm-workspace.yaml             # pnpm workspace (web + packages/types)
├── turbo.json                      # Turborepo: orchestrates pnpm + cargo tasks
├── docker-compose.yml
└── openapi.yaml                    # Generated by the API server at build time
```

---

## Key Technology Choices (Rust Layer)

### Web Framework: Axum

Axum is the recommended framework for both `api` and `mcp` crates.

- Built on Tokio — the de facto Rust async runtime
- Tower middleware ecosystem — authentication, rate limiting, tracing are all composable middleware layers
- Excellent `tracing` integration for structured logging
- `utoipa` integrates directly with Axum for OpenAPI schema generation

### LLM Integration

| Provider | Library | Notes |
|----------|---------|-------|
| OpenAI | `async-openai` | Mature, actively maintained, supports streaming and embeddings |
| Anthropic | `anthropic-sdk-rs` or raw `reqwest` | Less mature; `reqwest` with manual serialization is reliable fallback |
| Ollama (v1.1) | `ollama-rs` or raw `reqwest` | Ollama has a simple REST API — direct HTTP is sufficient |

Wrap all LLM calls behind a `LlmClient` trait defined in a shared `crates/llm` crate, so the provider can be swapped by configuration.

### Markdown AST Parsing: comrak

`comrak` is the right choice for structure-aware chunking in Rust:
- CommonMark-compliant with GitHub Flavored Markdown extensions
- Exposes a full AST (`Arena`-based) — heading nodes, paragraph nodes, code blocks, and tables are all first-class AST nodes
- Heading boundary traversal is straightforward with the AST walker
- Actively maintained

### Database Clients

| Store | Library | Notes |
|-------|---------|-------|
| PostgreSQL | `sqlx` | Async, compile-time query checking, no ORM overhead |
| VexFS | VexFS Rust client (first-party) | Direct integration with the VexFS team |

### OpenAPI Type Contract

The API server uses `utoipa` to generate an `openapi.yaml` at build time. The Turborepo pipeline runs `openapi-typescript` against this spec to generate the `packages/types/generated/` TypeScript types consumed by the Next.js frontend. This is the single source of truth for the API contract.

```
cargo build (crates/api)
  → generates openapi.yaml
    → openapi-typescript
      → packages/types/generated/*.ts
        → apps/web consumes types
```

---

## Turborepo Pipeline Configuration

```json
// turbo.json
{
  "tasks": {
    "build:rust": {
      "cache": true,
      "inputs": ["crates/**/*.rs", "Cargo.toml", "Cargo.lock"],
      "outputs": ["target/release/api", "target/release/mcp", "openapi.yaml"]
    },
    "build:types": {
      "dependsOn": ["build:rust"],
      "cache": true,
      "inputs": ["openapi.yaml"],
      "outputs": ["packages/types/generated/**"]
    },
    "build": {
      "dependsOn": ["build:types"],
      "cache": true
    },
    "dev": {
      "persistent": true,
      "cache": false
    }
  }
}
```

---

## Consequences

**Easier:**
- MCP server p95 latency target is achievable without careful async tuning — Rust's performance characteristics make it the expected outcome, not an optimization target
- The chunker crate runs in its own thread pool — large pages with many sections do not block the API request handler
- Docker images for the Rust services are small single-binary containers — fast pull times on install
- Rust's type system catches data contract mismatches at compile time — fewer runtime surprises in the chunking pipeline

**Harder:**
- Local development setup is more involved — contributors need both the Rust toolchain (`rustup`) and Node.js/pnpm installed
- The OpenAPI codegen step adds a dependency between the Rust build and the TypeScript build — Turborepo's task graph manages this, but it must be kept up to date
- Rust compile times slow the inner development loop, especially for first-time builds — use `cargo-watch` for incremental rebuilds during development
- Open-source contributions to the backend require Rust familiarity — consider maintaining thorough contributor documentation and ensuring the chunker and MCP crates have clear, well-commented interfaces

**Must revisit:**
- If VexFS does not yet have a Rust client library, the first engineering task is to build or request one — this is a blocking dependency for the backend services
- Evaluate `cargo-chef` for Docker layer caching of Rust dependencies — without it, Docker builds recompile all dependencies on every code change

---

## Action Items

1. [ ] Confirm VexFS has a Rust client library or plan its development with the VexFS team
2. [ ] Initialize Cargo workspace at the repo root with `crates/api`, `crates/mcp`, `crates/chunker`, `crates/db`
3. [ ] Set up `utoipa` in `crates/api` and validate OpenAPI schema generation as part of the build
4. [ ] Configure `openapi-typescript` in the Turborepo pipeline to consume `openapi.yaml` and output `packages/types/generated/`
5. [ ] Set up `cargo-watch` for hot reload during local Rust development
6. [ ] Set up `cargo-chef` in the Docker build for dependency layer caching
7. [ ] Document the full local development setup (Rust toolchain + pnpm) in `CONTRIBUTING.md`
8. [ ] Validate `async-openai` and an Anthropic HTTP client against the workspace API key model
