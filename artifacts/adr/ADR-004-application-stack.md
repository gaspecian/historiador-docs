# ADR-004: Application Stack — Node.js / TypeScript

**Status:** Superseded by [ADR-006](ADR-006-application-stack-rust.md)
**Date:** 2026-04-08
**Superseded:** 2026-04-08
**Deciders:** Gabriel Specian (Nexian Tech)

> ⚠️ This ADR has been superseded. The backend stack was changed from Node.js/TypeScript (Fastify) to Rust (Axum). See ADR-006 for the current decision.

---

## Context

Historiador Doc consists of three runtime components: the frontend dashboard (Next.js), the backend API server, and the MCP server. A decision is needed on the language and framework for the backend API and MCP server, as the frontend is already established as Next.js (React).

Selecting a single language across all three components has compounding benefits for an open-source project: shared tooling, consistent code style, shared type definitions, and a lower barrier to contribution.

Key constraints:
- Must have strong LLM SDK support (OpenAI, Anthropic, Ollama)
- Must run efficiently in a Docker container on 2 vCPU / 4GB RAM
- Must have a markdown AST parsing library suitable for structure-aware chunking
- Must allow sharing types between API and MCP server (monorepo setup)
- Must have a healthy open-source ecosystem for long-term maintainability

---

## Decision

**Use Node.js with TypeScript across all backend services** (API server and MCP server), with **Fastify** as the HTTP framework. The frontend remains Next.js (React). The full project is structured as a TypeScript monorepo using `pnpm workspaces`.

LLM integration uses the **Vercel AI SDK** as the primary abstraction layer, providing a unified interface for OpenAI, Anthropic, and (in v1.1) Ollama.

---

## Options Considered

### Option A: Node.js / TypeScript (Fastify) ✅ Selected

| Dimension | Assessment |
|-----------|------------|
| Language consistency | Excellent — same language as Next.js frontend |
| LLM SDK support | Excellent — Vercel AI SDK, LangChain.js, native SDKs |
| Markdown parsing | Excellent — `remark` ecosystem (AST-based) |
| Performance | High — Fastify is the fastest Node.js HTTP framework |
| Type sharing | Native — shared TypeScript types across monorepo |
| Open-source contribution barrier | Low — TypeScript is widely known |
| Docker footprint | Low — Node.js images are lean |

**Pros:**
- End-to-end TypeScript means shared type definitions across the frontend, API, and MCP server — the `Chunk`, `Page`, `User`, and `MCPResponse` types are defined once and used everywhere
- The Vercel AI SDK provides a single `generateText` / `streamText` interface that abstracts OpenAI and Anthropic behind identical function signatures — switching LLM providers requires changing one configuration value
- `remark` and the unified.js ecosystem are the de facto standard for markdown AST processing in Node.js — essential for structure-aware chunking
- Fastify provides significantly better throughput than Express with lower memory overhead, which matters for the MCP server's p95 latency target
- pnpm workspaces give a clean monorepo structure with shared packages (`packages/db`, `packages/types`, `packages/chunker`)
- The open-source community for TypeScript tooling (ESLint, Vitest, tsup) is large and actively maintained

**Cons:**
- Node.js single-threaded event loop can be a bottleneck for CPU-intensive tasks (embedding generation, chunking) — must use worker threads or async patterns to avoid blocking
- TypeScript adds a build step — contributors must run `tsc` or `tsup` before running services

---

### Option B: Python Backend (FastAPI) + TypeScript Frontend

| Dimension | Assessment |
|-----------|------------|
| Language consistency | Poor — two languages, two ecosystems |
| LLM SDK support | Excellent — Python LangChain, OpenAI SDK are mature |
| Markdown parsing | Good — `mistune`, `python-markdown` |
| Performance | Good — FastAPI is async and performant |
| Type sharing | None — types cannot be shared between Python and TypeScript |
| Open-source contribution barrier | Medium — requires knowledge of both ecosystems |
| Docker footprint | Medium — Python images are larger than Node |

**Pros:**
- Python has the most mature AI/ML ecosystem — LangChain, LlamaIndex, sentence-transformers, and all embedding libraries are Python-first
- Many AI developers are more familiar with Python

**Cons:**
- Two languages means two linting configs, two test frameworks, two CI pipelines, and two Docker base images
- No type sharing between the Python API and TypeScript frontend — requires manual synchronization of shared types or a code generation step (e.g., openapi-typescript)
- Python's AI ecosystem advantage is largely neutralized for Historiador Doc's use case: the Vercel AI SDK covers OpenAI and Anthropic well, and `remark` handles markdown AST parsing cleanly
- Higher contribution barrier for the open-source community — contributors need to know both Python and TypeScript
- Python images are significantly larger, increasing the Docker Compose startup time and disk footprint on the minimum-spec installation target

**Rejected** — the two-language penalty is not justified by Python's AI ecosystem advantage in this specific use case.

---

### Option C: Go Backend + TypeScript Frontend

| Dimension | Assessment |
|-----------|------------|
| Language consistency | Poor — two languages |
| LLM SDK support | Limited — Go LLM SDKs are less mature |
| Markdown parsing | Good — `goldmark` |
| Performance | Excellent — compiled, multi-threaded |
| Type sharing | None |
| Open-source contribution barrier | High — Go is less universally known |

**Pros:**
- Exceptional performance and low memory footprint
- Native concurrency model handles CPU-intensive chunking without worker threads

**Cons:**
- Go LLM SDK ecosystem is significantly less mature than Node.js or Python
- Very high open-source contribution barrier
- Two-language penalty, same as Option B
- Performance advantage is not necessary at v1 workloads

**Rejected** — LLM SDK immaturity and high contribution barrier disqualify it for this project.

---

## Trade-off Analysis

The decisive factor is the **monorepo type-sharing story**. Historiador Doc's domain model — `Page`, `Chunk`, `Collection`, `MCPQuery`, `MCPResponse` — is used in the frontend (rendering), the API (storage and retrieval), and the MCP server (serving). In a single-language TypeScript monorepo, these types are defined once in `packages/types` and imported everywhere. In a two-language setup, they must be maintained in two places or generated via an OpenAPI schema — a persistent maintenance tax.

Python's AI ecosystem advantage is real in general, but not for Historiador Doc's specific requirements. The Vercel AI SDK handles OpenAI and Anthropic with the same interface quality as the Python SDKs. `remark` handles markdown AST parsing idiomatically. VexFS has or will have a Node.js client maintained by the same team.

---

## Monorepo Structure

```
historiador-doc/
├── apps/
│   ├── web/                    # Next.js frontend (port 3000)
│   ├── api/                    # Fastify API server (port 3001)
│   └── mcp/                    # MCP server (port 3002)
├── packages/
│   ├── types/                  # Shared TypeScript interfaces and enums
│   ├── db/                     # PostgreSQL client + VexFS client (shared)
│   ├── chunker/                # Structure-aware markdown chunker
│   └── llm/                    # Vercel AI SDK wrapper (LLM abstraction)
├── docker-compose.yml
├── pnpm-workspace.yaml
└── turbo.json                  # Turborepo for build orchestration
```

---

## Consequences

**Easier:**
- A single `pnpm install` at the monorepo root installs all dependencies
- TypeScript interfaces are the source of truth for all API contracts — no separate OpenAPI spec required in v1
- The chunker package can be unit-tested independently of the API and MCP services
- Contributors only need to know TypeScript to contribute to any part of the codebase

**Harder:**
- CPU-intensive operations (embedding generation, chunking large pages) must be carefully managed to avoid blocking the Node.js event loop — use `worker_threads` or delegate to an async queue for large batches
- TypeScript build configuration across multiple packages requires careful `tsconfig` path alias setup

**Must revisit:**
- If embedding generation at scale becomes a bottleneck, evaluate offloading the embedding pipeline to a dedicated worker service (still in TypeScript, but isolated)
- If Ollama support (v1.1) requires local model inference, validate that the Vercel AI SDK's Ollama adapter meets Historiador Doc's needs before committing

---

## Action Items

1. [ ] Initialize the monorepo with `pnpm workspaces` and Turborepo
2. [ ] Define `packages/types` with core domain interfaces: `Page`, `Chunk`, `Collection`, `User`, `MCPQuery`, `MCPResponse`
3. [ ] Configure Fastify with TypeScript in `apps/api` with route-level type safety
4. [ ] Set up Vercel AI SDK in `packages/llm` with OpenAI and Anthropic providers; validate both work with the workspace API key model
5. [ ] Configure ESLint, Prettier, and Vitest once at the monorepo root — all packages inherit the same configuration
