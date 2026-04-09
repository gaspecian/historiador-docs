# Historiador Doc — Product Requirements Document

**Version**: 1.1 (Open Questions Resolved)
**Status**: Draft
**Author**: Gabriel Specian, Nexian Tech
**Last Updated**: 2026-04-08

> **Changelog v1.1**: All open questions resolved. Language configuration promoted to P0. VexFS confirmed as vector store. Structure-aware chunking adopted. Nested collections confirmed for v1. MCP discrete responses confirmed. Workspace-level API key confirmed.

---

## Problem Statement

Companies accumulate knowledge in scattered, inconsistent places — wikis nobody reads, docs nobody updates, and tribal knowledge that lives only in people's heads. Existing documentation tools (Confluence, Notion, GitBook) were designed for humans to read, not for AI to consume. As AI assistants become central to how employees and developers work, this creates a critical gap: the company's knowledge is invisible to the AI tools people use every day.

The result is that employees ask AI tools questions that should have definitive answers — and get hallucinations or generic responses instead of the company's actual policy, process, or product information. Documentation remains an afterthought because the effort to create it is high and the payoff is unclear.

Historiador Doc solves this by making documentation creation effortless through AI-assisted authoring and making every knowledge base natively accessible to AI tools via a built-in MCP server — without requiring any integration work.

---

## Goals

1. **Lower the barrier to creation**: Any employee — regardless of technical skill — can create and publish a documentation page in under 10 minutes through a conversational AI interface.
2. **Make documentation AI-accessible from day one**: Every Historiador Doc workspace ships with an MCP endpoint that any AI tool can query immediately after installation, with zero additional configuration.
3. **Serve both internal and external use cases**: The same platform supports internal company knowledge bases (HR policies, engineering runbooks, onboarding guides) and external product documentation (developer docs, help centers, API references).
4. **Keep data inside the company**: As a self-hosted open-source system, all documentation, query data, and vector embeddings stay within the company's own infrastructure.
5. **Enable IT to deploy and manage without ongoing maintenance burden**: IT can complete a full installation and onboarding in under one hour, including language configuration.

---

## Non-Goals (v1)

- **Gap detection and query analytics** — The system will not log or analyze MCP queries to surface documentation gaps in v1. This is the highest-priority feature for v2, but requires a stable content and MCP layer first.
- **Real-time collaborative editing** — Multiple users editing the same page simultaneously is out of scope. Single-author editing with version history is sufficient for v1.
- **Multi-tenant SaaS offering** — v1 is self-hosted only. A managed cloud tier is a future business model consideration, not a v1 deliverable.
- **Fine-tuning or training LLMs on company docs** — Historiador Doc uses LLMs for authoring assistance and chunking; it does not train or fine-tune models on company content.
- **Public-facing documentation hosting** — v1 does not include a branded public documentation site. Docs can be exported as markdown but Historiador Doc does not serve as a public CDN.
- **Per-author LLM cost controls** — All authors share the workspace-level API key in v1. Token quotas and per-user cost tracking are a v2 concern.

---

## User Personas

### 1. IT Admin
Installs, configures, and manages the Historiador Doc instance. Sets up user access, configures the LLM API key, defines the workspace language configuration, and manages the MCP endpoint. Technical, but not necessarily a developer. Needs a fast, reliable setup process and minimal ongoing maintenance.

### 2. Documentation Author (any employee)
Creates and maintains documentation pages. Could be an HR manager, a customer support lead, a developer, or an operations coordinator. Non-technical. Needs a creation experience that feels as easy as writing a message, not authoring a document. Must be able to write in any of the languages configured for their workspace.

### 3. AI Tool User (end consumer)
Any person (or automated system) using an AI tool — Claude, ChatGPT, Cursor, a custom chatbot — that has been pointed at the Historiador Doc MCP endpoint. Their experience is entirely mediated by the AI tool; they never interact with Historiador Doc directly. What they care about is whether the AI gives them accurate, relevant answers.

---

## User Stories

### IT Admin

- As an IT admin, I want to install Historiador Doc using a single Docker command so that I can have a working instance running in under an hour without reading extensive documentation.
- As an IT admin, I want to configure the workspace LLM API key once during installation so that all authors share a single connection without each needing their own key.
- As an IT admin, I want to define which languages documentation must be written in during installation so that the platform enforces multilingual documentation standards from day one.
- As an IT admin, I want to invite users by email and assign them roles (author, viewer, admin) so that I can control who can create and edit documentation.
- As an IT admin, I want to access the MCP endpoint URL and authentication token from the admin dashboard so that I can connect AI tools to the knowledge base without engineering support.
- As an IT admin, I want to export all documentation as a zip of markdown files so that I can back up or migrate the knowledge base at any time.

### Documentation Author

- As a documentation author, I want to describe what I need to document in plain language and have the AI draft a structured page for me so that I can create documentation without starting from a blank page.
- As a documentation author, I want the AI editor to prompt me in my configured workspace language(s) so that I naturally produce documentation in the required language(s).
- As a documentation author, I want to review, edit, and approve the AI-generated draft before publishing so that I remain in control of the content that goes live.
- As a documentation author, I want to organize pages into nested collections (e.g., "Engineering > Backend > APIs") so that the knowledge base has a deep, navigable structure that mirrors how our organization is structured.
- As a documentation author, I want to save a page as a draft before publishing so that I can work on it across multiple sessions without making it available prematurely.
- As a documentation author, I want to edit an existing page and have the AI update the structured/chunked representation automatically so that I do not need to manually maintain two versions.
- As a documentation author, I want to search across all published pages so that I can find existing documentation before creating a duplicate.

### AI Tool User (via MCP)

- As an AI tool user, I want the AI assistant I use daily to answer questions about my company's policies and processes accurately so that I do not need to search through a wiki manually.
- As a developer, I want to point my coding assistant at the product's MCP endpoint so that it can answer questions about the API, internal tools, and engineering standards without me pasting docs into the context window.

---

## Requirements

### Must-Have — P0 (v1 cannot ship without these)

---

#### AI Conversational Editor

A chat-style interface where the user describes what they want to document in natural language. The AI asks clarifying questions as needed, then generates a structured, human-readable markdown page. The editor operates in the workspace's configured language(s).

**Acceptance criteria**:
- [ ] User can type a documentation brief (e.g., "Document our employee onboarding process") and receive a draft page within 30 seconds
- [ ] The AI asks at least one clarifying question before generating the draft when the brief is ambiguous
- [ ] The generated draft is structured with headings, sections, and clear prose — not bullet-point soup
- [ ] The user can iterate on the draft by sending follow-up messages (e.g., "Add a section on equipment setup")
- [ ] The user can switch to a direct markdown editor at any point in the session
- [ ] If the workspace is configured for multiple languages, the AI editor prompts the author to create content in each configured language, or flags which language versions are missing before publish

---

#### Structure-Aware Chunking (Dual Representation Layer)

When a page is published, the system automatically generates a chunked representation optimized for semantic retrieval. Chunking is **structure-aware**: the system respects markdown document structure — headings, sections, lists, and code blocks — when dividing content into discrete knowledge units. Raw fixed-size splitting is not used.

Each chunk maps to a logical unit of meaning within the document (e.g., one section, one procedure, one concept). This representation is stored in VexFS and is never shown directly to authors.

**Acceptance criteria**:
- [ ] On publish, the system parses the markdown page and splits it into chunks aligned with heading boundaries (H1 → H2 → H3), never splitting mid-section
- [ ] If a section exceeds a configurable maximum chunk size (default: 512 tokens), it is split at paragraph boundaries, not mid-paragraph
- [ ] Each chunk is stored with metadata: source page ID, section heading, heading hierarchy path, language, last updated timestamp, author ID
- [ ] Updates to a published page trigger automatic re-chunking of the full page; stale chunks are replaced atomically
- [ ] The chunking process runs asynchronously and does not block the publish action
- [ ] Chunking supports all configured workspace languages; chunk metadata includes a `language` field

---

#### MCP Server Endpoint

Each workspace has a dedicated MCP-compatible server endpoint that AI tools can query to retrieve knowledge from the documentation base. Responses are **discrete chunk responses** (not streaming) in v1.

**Acceptance criteria**:
- [ ] The MCP endpoint is available immediately after installation with no additional configuration
- [ ] The endpoint accepts natural language queries and returns the most relevant chunks as discrete responses, with source attribution (page title, section heading, collection path, page URL)
- [ ] Each response includes a confidence/relevance score per chunk
- [ ] The endpoint is protected by a configurable bearer token, generated during installation
- [ ] The endpoint follows the MCP protocol spec and is compatible with Claude, Cursor, and standard MCP clients
- [ ] Response latency is under 2 seconds for 95% of queries on a standard installation
- [ ] Streaming is explicitly not supported in v1; the endpoint returns the full response once retrieval is complete

---

#### Nested Collections

Pages are organized into a nested collection hierarchy (folders within folders). There is no depth limit enforced by the system, though the recommended UX guides users toward three levels maximum.

**Acceptance criteria**:
- [ ] Authors can create collections at the root level or as children of any existing collection
- [ ] Pages can be placed in any collection at any depth
- [ ] The dashboard sidebar renders the collection tree with expand/collapse controls
- [ ] Collections can be renamed and moved (including all child pages and sub-collections)
- [ ] Deleting a collection prompts the user to either delete all contents or move pages to another collection
- [ ] The MCP response includes the full collection path for each chunk (e.g., `Engineering / Backend / APIs`) to give AI tools structural context

---

#### Language Configuration

Language support is defined at installation time by IT. The configuration specifies which language(s) documentation must be produced in. This is a global workspace setting, not a per-page or per-author setting.

**Acceptance criteria**:
- [ ] During installation (first-run setup wizard), the admin selects one or more languages for the workspace from a supported language list (all languages supported by the configured LLM)
- [ ] Once set, the language configuration is stored in the workspace settings and is not changeable by non-admin users
- [ ] The AI editor uses the configured language(s) for all generated drafts
- [ ] If multiple languages are configured, the editor prompts the author to produce a version in each language; a page is not considered "complete" until all required language versions exist (the system warns but does not block publish in v1)
- [ ] The MCP endpoint accepts queries in any language and returns chunks in the language(s) that best match the query; chunk metadata includes a `language` field to allow AI tools to filter by language
- [ ] Language configuration can be updated by an admin post-installation from the admin panel, with a warning that existing pages will not be retroactively re-evaluated

---

#### Page Management Dashboard

A web dashboard where authors can create, view, edit, organize, and publish documentation pages.

**Acceptance criteria**:
- [ ] Authors can create a new page via the AI editor or a blank markdown editor
- [ ] Pages have draft and published states
- [ ] Pages are organized within the nested collection hierarchy
- [ ] Authors can search published and draft pages by title and content
- [ ] Each page shows last edited date, author, and language(s) available
- [ ] Pages missing required language versions are visually flagged in the dashboard

---

#### User & Access Management

IT admin can manage users, roles, and workspace configuration from an admin panel.

**Acceptance criteria**:
- [ ] Admin can invite users by email
- [ ] Three roles: Admin (full access), Author (create/edit pages), Viewer (read-only dashboard access)
- [ ] Admin configures the workspace-level LLM API key (supports OpenAI and Anthropic at launch)
- [ ] Admin can view and regenerate the MCP endpoint bearer token
- [ ] Admin can deactivate user accounts
- [ ] Admin can view and update the workspace language configuration

---

#### Self-Hosted Deployment (Docker Compose)

The full system deploys via Docker Compose with a single command. The first-run setup wizard collects LLM API key, language configuration, and admin account details before the workspace is usable.

**Acceptance criteria**:
- [ ] `docker compose up` starts all required services: web app, API server, MCP server, VexFS, PostgreSQL
- [ ] On first run, a setup wizard runs in the browser to collect: admin email/password, LLM provider and API key, workspace language(s)
- [ ] Installation documentation covers setup in under 30 minutes for a sysadmin familiar with Docker
- [ ] The system runs on a standard Linux VPS with 2 vCPU / 4GB RAM minimum
- [ ] All data persists across container restarts via named Docker volumes

---

### Nice-to-Have — P1 (high-priority fast follows)

- **Page version history**: Authors can view previous versions of a page and restore any version. Important for compliance-sensitive documentation (HR policies, legal procedures).
- **Ollama / local LLM support**: IT can configure a local model endpoint (Ollama) instead of a cloud API key. Critical for air-gapped environments and organizations with strict data residency policies.
- **Rich media in pages**: Authors can embed images and code blocks with syntax highlighting. Code blocks should be chunk-preserved (not split mid-block) by the structure-aware chunker.
- **MCP endpoint usage analytics**: Admin can see basic metrics — total queries this month, most queried topics, queries with no matching result (precursor to gap detection in v2).
- **Markdown export**: Authors and admins can export any page or the full knowledge base as a zip of markdown files, organized by collection hierarchy.
- **Configurable public pages**: Individual pages or collections can be marked public (accessible via MCP without bearer token authentication) for external-facing product documentation use cases.

---

### Future Considerations — P2 (architect for, build in v2)

- **Gap detection flywheel**: The MCP layer logs queries that return low-confidence or no results. These are clustered by topic and surfaced as documentation requests. Authors receive a weekly digest of unanswered questions and can create a new page pre-filled with the question as a brief.
- **SSO / SAML integration**: Enterprise authentication via Okta, Azure AD, or Google Workspace.
- **Multi-workspace support**: A single installation hosts multiple isolated workspaces — useful for agencies or holding companies managing documentation for multiple clients or brands.
- **Webhook notifications**: On page publish or update, notify external systems (Slack, email) so teams stay informed of knowledge base changes.
- **Public documentation site**: A branded, publicly accessible documentation portal generated from published pages, with custom domain support.
- **Per-author LLM token quotas**: Admin can set monthly token limits per author to control LLM API costs at scale.

---

## Success Metrics

### Leading Indicators (first 30 days post-launch)

| Metric | Target | Measurement |
|--------|--------|-------------|
| Time to first published page (from account creation) | < 10 minutes | Timestamp: account created → first page published |
| % of new users publishing at least one page in week 1 | > 60% | Cohort analysis by signup week |
| IT installation time (Docker up → first user invited) | < 60 minutes | Setup wizard completion timestamp |
| MCP query success rate (≥1 chunk returned) | > 75% | MCP server response logs |
| AI editor adoption (% of pages started via AI editor vs blank) | > 80% | Page creation event source tracking |

### Lagging Indicators (60–90 days post-launch)

| Metric | Target | Measurement |
|--------|--------|-------------|
| Monthly active documentation contributors | Growing week-over-week | DAU/MAU by Author role |
| Average pages per workspace at 60 days | > 20 pages | Workspace content snapshots |
| MCP queries per workspace per month | > 100 queries | MCP server logs |
| User-reported documentation quality | > 7/10 | In-app prompt at 30 days post-install |

---

## Resolved Decisions

The following questions were open in v1.0 of this spec and have been resolved:

| # | Question | Decision |
|---|----------|----------|
| 1 | Chunking strategy | **Structure-aware** — chunks follow heading and section boundaries; paragraph splits as fallback for oversized sections |
| 2 | MCP streaming vs discrete responses | **Discrete** — full response returned once retrieval is complete; streaming not supported in v1 |
| 3 | LLM API key scope | **Workspace-level** — one API key configured by IT admin at installation; shared by all authors |
| 4 | Vector database | **VexFS** ([github.com/lspecian/vexfs](https://github.com/lspecian/vexfs)) — bundled in Docker Compose; replaces pgvector/Qdrant as the default vector store |
| 5 | Collection hierarchy | **Nested** — unlimited depth; dashboard recommends three levels but does not enforce a cap |
| 6 | Language support | **Global configuration at installation** — IT admin defines required workspace language(s) in the setup wizard; applies to all authors and AI editor prompts |

---

## Architecture Notes

Directional guidance for the engineering team — not prescriptive requirements. See ADRs 001–006 in `adr/` for full decision rationale.

**Recommended stack**:

| Layer | Technology | Notes |
|-------|------------|-------|
| Frontend | Next.js (React, TypeScript) | Dashboard, AI editor, admin panel, setup wizard |
| Backend API | Rust + Axum | Page management, user management, chunking pipeline, language validation |
| MCP Server | Rust + Axum (standalone service) | Read-only; implements MCP protocol; network-isolated from the API |
| Chunker | Rust (`comrak` AST) | Lives in `crates/chunker`; shared between API and MCP crates |
| Vector store | **VexFS** | First-party vector database; bundled in Docker Compose |
| Relational DB | PostgreSQL (`sqlx`) | Pages, users, collections, metadata, language config |
| LLM integration | `async-openai` + Anthropic HTTP | Wrapped behind a `LlmClient` trait in `crates/llm`; Ollama added in v1.1 |
| API contract | `utoipa` (Rust) → `openapi-typescript` | OpenAPI schema generated from Rust; TypeScript types auto-generated for frontend |

**Monorepo structure**:

```
historiador-doc/
├── apps/web/               # Next.js frontend
├── packages/types/         # Auto-generated TypeScript types (from OpenAPI)
├── crates/
│   ├── api/                # Axum API server (port 3001)
│   ├── mcp/                # Axum MCP server (port 3002)
│   ├── chunker/            # Structure-aware markdown chunker
│   ├── db/                 # Shared VexFS + PostgreSQL clients
│   └── llm/                # LLM provider abstraction
├── Cargo.toml              # Rust workspace root
├── pnpm-workspace.yaml     # pnpm workspace (frontend + types)
└── turbo.json              # Turborepo: orchestrates both cargo and pnpm tasks
```

**Key architectural decisions** (see ADR files for full rationale):

**Structure-aware chunker** (`comrak`, Rust): parses the markdown AST at heading boundaries. Code blocks, tables, and lists are atomic AST nodes — never split. Output is a `Vec<Chunk>` where each chunk carries `{ heading_path, content, token_count, language }`. Written to VexFS on publish.

**VexFS as vector store**: the API writes chunk embeddings to VexFS on publish/update; the MCP server reads from VexFS on every query. VexFS is the retrieval source of truth; PostgreSQL is the content and metadata store.

**MCP server isolation**: standalone Rust service with no write path. Companies expose only port 3002 externally; ports 3000 (frontend) and 3001 (API) remain internal. See ADR-003 for network topology.

**Language at the chunk level**: every chunk written to VexFS carries a `language` field (BCP 47). The MCP server supports optional `language` filter on queries. Workspace language configuration is set at installation time by IT admin.

---

## Timeline Considerations

No hard external deadlines. Recommended phasing:

**Alpha — internal / early testers**
- AI editor with markdown output
- Page storage, nested collections, and dashboard
- Docker Compose deployment with setup wizard (LLM key + language config)
- Target: a single team can use it daily to build a real knowledge base

**Beta — public open-source release**
- MCP endpoint live and tested against Claude, Cursor, and a custom client
- Structure-aware chunking via VexFS
- Language configuration enforced in editor and chunker
- User management and roles
- Public GitHub repository with contribution guidelines and setup documentation

**v1.0 — stable release**
- All P0 requirements complete and tested
- Performance targets met (MCP response < 2s p95)
- Installation documentation tested by a non-author completing setup in < 60 minutes
- Ollama support (P1) included if timeline allows

---

## Appendix: Glossary

| Term | Definition |
|------|------------|
| **MCP** | Model Context Protocol — an open standard for connecting AI tools to external data sources and services |
| **Chunk** | A discrete, retrievable unit of knowledge extracted from a documentation page — one logical section, procedure, or concept |
| **Structure-aware chunking** | A chunking strategy that splits content at heading and section boundaries (respecting the document's markdown structure) rather than at fixed token counts |
| **Dual representation** | The system's internal separation between a human-readable markdown page and its AI-optimized chunked equivalent stored in VexFS |
| **VexFS** | The vector database used by Historiador Doc to store and query chunk embeddings ([github.com/lspecian/vexfs](https://github.com/lspecian/vexfs)) |
| **Gap detection** | A v2 feature where the MCP layer identifies topics that users query but that lack adequate documentation coverage, surfacing them as documentation requests |
| **BYOK** | Bring Your Own Key — the pattern where the organization provides its own LLM API key; in Historiador Doc this is a workspace-level key configured by IT |
| **Language configuration** | A global workspace setting defined at installation that specifies which language(s) all documentation must be produced in |
