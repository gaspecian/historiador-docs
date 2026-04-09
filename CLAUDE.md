# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Repository State

This repository currently contains **planning artifacts only** — no code has been scaffolded yet. The entire tree is:

- [artifacts/product/historiador-doc-prd.md](artifacts/product/historiador-doc-prd.md) — PRD v1.1 (source of truth for requirements)
- [artifacts/adr/](artifacts/adr/) — ADRs 001–006 (accepted architectural decisions)
- [artifacts/sprints/sprint-01.md](artifacts/sprints/sprint-01.md) — Sprint 1 foundation plan

Before making any architectural suggestion or writing code, read the relevant ADR — these decisions are already settled and should not be relitigated without explicit user direction. **ADR-006 supersedes ADR-004** (Rust backend was chosen over Node.js).

## What Is Being Built

Historiador Doc is a self-hosted, open-source documentation platform where every knowledge base ships with a built-in MCP server. Two unusual things distinguish it from typical docs tools:

1. **Dual representation**: pages are authored as human-readable markdown *and* stored as a separate "chunked representation" in a vector store. Authors never see the chunks; AI tools never see the raw markdown. The chunker is the bridge.
2. **MCP-native from day one**: the MCP endpoint is a first-class service, not a plugin. It runs as a **standalone, read-only process** (ADR-003) so companies can expose only its port externally while keeping the authoring app internal.

## Architecture (Planned)

**Mixed-language monorepo**: Rust backend (Cargo workspace) + Next.js frontend (pnpm workspace), orchestrated by Turborepo. See [ADR-006](artifacts/adr/ADR-006-application-stack-rust.md) for the full rationale.

```
historiador-doc/
├── apps/web/              # Next.js frontend (dashboard, AI editor, admin, setup wizard)
├── packages/types/        # TypeScript types auto-generated from openapi.yaml
├── crates/
│   ├── api/               # Axum API server         (port 3001, internal)
│   ├── mcp/               # Axum MCP server         (port 3002, externally exposed)
│   ├── chunker/           # Structure-aware markdown chunker (comrak AST)
│   ├── db/                # Shared VexFS + PostgreSQL clients
│   └── llm/               # LlmClient trait + OpenAI/Anthropic/Ollama impls
├── Cargo.toml             # Rust workspace root
├── pnpm-workspace.yaml
└── turbo.json
```

**Critical cross-cutting invariants** (violating these breaks the architecture):

- **MCP server has zero write access.** It reads VexFS and PostgreSQL only. This is a security boundary, not a convention — the Docker Compose config must not pass write credentials to the MCP container. See ADR-003.
- **VexFS is the retrieval source of truth; PostgreSQL is the content/metadata source of truth.** Chunk embeddings live in VexFS; page markdown, users, collections, and language config live in PostgreSQL. Do not duplicate. See ADR-001.
- **Chunks are structure-aware, never fixed-size.** The chunker walks the markdown AST at heading boundaries (H1→H2→H3) and never splits mid-section. Code blocks, tables, and lists are atomic AST nodes. Paragraph boundaries are the only fallback for oversized sections. See ADR-002 for the full algorithm.
- **Every chunk carries a `language` field (BCP 47).** Language is a workspace-level setting configured at installation time, not per-page or per-author. The `page_versions` table is keyed by `(page_id, language)`. See ADR-005.
- **OpenAPI is the single source of truth for the API contract.** `crates/api` uses `utoipa` to emit `openapi.yaml` at build time; Turborepo runs `openapi-typescript` against it to generate `packages/types/generated/`. Never hand-edit generated types; never define an API route without a `utoipa` annotation.

## Planned Development Commands

These commands come from [sprint-01.md](artifacts/sprints/sprint-01.md) and ADR-006. They will become live once Sprint 1 scaffolds the workspace — until then they are the target shape, not runnable:

- `docker compose up` — start the full stack (web, api, mcp, postgres, vexfs)
- `cargo build` / `cargo test` / `cargo clippy --deny warnings` — Rust workspace build/test/lint (must pass zero warnings in CI)
- `pnpm install` / `pnpm build` — TypeScript workspace
- `sqlx-cli migrate run` — apply PostgreSQL migrations (migrations live in `crates/db/migrations/`)
- `turbo build` — orchestrates both `cargo` and `pnpm` pipelines with OpenAPI codegen wired into the task graph (`build:rust` → `build:types` → `build`)

Docker builds should use `cargo-chef` for Rust dependency layer caching — set this up in the Dockerfile from the start, not as a later optimization (see sprint-01 Risk 3).

## Conventions When Modifying Artifacts

- ADRs are **append-only history**. To change a decision, write a new ADR that supersedes the old one (as ADR-006 did to ADR-004). Do not edit accepted ADRs retroactively.
- The PRD's "Resolved Decisions" table is load-bearing — if a decision changes, update both the table and any affected ADR.
- Sprint files are snapshots of a week's plan; don't rewrite history to match actual outcomes. Write a retro or a new sprint instead.
