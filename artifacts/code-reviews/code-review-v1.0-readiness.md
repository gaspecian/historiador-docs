# Historiador Doc — Code Review vs PRD (v1.0 Readiness)

**Reviewer:** Claude (code review pass)
**Date:** 2026-04-18
**Scope:** Full monorepo (`historiador-docs`) compared against `historiador-doc-prd.md` (v1.2), Sprints 1–9, and ADR-001 through ADR-008
**Branch:** `main` at HEAD (no tags; no `v1.0.0`)
**Verdict:** **Request changes — do NOT tag `v1.0.0` in current state.** Feature coverage is strong (Sprints 1–8 substantially complete), but Sprint 9 was abandoned and several P0 PRD invariants are not yet honored by the implementation.

---

## 1. Executive Summary

The codebase reflects the architecture described in the PRD at a structural level: the Axum API, standalone read-only MCP binary, structure-aware chunker, multilingual editor, version history, Ollama support, and markdown export all exist and are wired end-to-end. Sprints 1–8 are substantially delivered.

However, **four categories of gaps prevent a responsible v1.0.0 tag today**:

1. **Sprint 9 (release hardening) never completed.** The repo has no `v1.0.0` tag, no `CONTRIBUTING.md`, no `CHANGELOG.md`, no `docs/performance.md`, and no evidence of a load test. The only Sprint 9 commit is `sprint 9 monday`.
2. **Two PRD contract promises are implemented the wrong way.** The editor uses **SSE instead of the WebSocket split-pane specified by ADR-008**, and the MCP endpoint is a **custom REST handler instead of JSON-RPC 2.0 over the MCP protocol**. Both work functionally, but neither honors the contract Claude Desktop and other MCP clients expect.
3. **One security primitive passes by accident.** The MCP bearer token is compared with `==` on byte slices; the PRD invariant and Sprint 9 hardening item both require a timing-safe comparison. It currently returns the correct value because `[u8]` equality short-circuits at the first mismatch, which is exactly the timing side-channel the PRD is trying to close.
4. **Operational readiness is incomplete.** There is no production docker-compose profile, RBAC is enforced at the use-case layer rather than at route middleware (correct today, fragile for future contributors), and Chronik is wired as optional with an in-memory fallback that silently loses data on container restart — a fact the README still documents as a known limitation but which is now contradicted by the recently-built version history feature.

None of these are deep architectural problems. All four are addressable inside a dedicated hardening sprint (likely a re-scoped Sprint 9).

---

## 2. Scorecard — PRD P0 vs. Implementation

| PRD P0 Requirement | Implementation Status | Notes |
|---|---|---|
| Self-hosted via docker-compose | 🟡 Partial | `docker-compose.yml` exists for dev (Postgres + Ollama). No production profile, no `docker-compose.prod.yml`, no documented hardening. |
| Axum REST API (`apps/api`, port 3001) | ✅ Complete | Clean architecture (domain/application/infrastructure/presentation) — cleaner than the PRD required. |
| Standalone read-only MCP binary (port 3002) | 🟡 Partial | Binary exists and uses `DATABASE_URL_READONLY`. **But the endpoint is custom REST, not JSON-RPC 2.0 MCP protocol.** Claude Desktop's MCP client will not be able to discover tools or call the endpoint as documented in the README. |
| Next.js 16 dashboard (port 3000) | ✅ Complete | Next.js 16 + React 19 + Tailwind 4. Auth, editor, admin panel all implemented. |
| Dual representation (markdown ↔ chunks) | ✅ Complete | Chunker (`crates/chunker`) walks the comrak AST; pages stored as markdown in Postgres; chunks persisted to Chronik. |
| Structure-aware chunks at heading boundaries | ✅ Complete | Walks H1→H2→H3; code blocks/tables/lists atomic. Minor: chunk metadata is missing `author_id` and `last_updated` fields. See finding 5.3. |
| Every chunk has BCP-47 `language` field | ✅ Complete | `page_versions` table keyed by `(page_id, language)`; chunks carry language. |
| MCP zero-write access (DB + env) | ✅ Complete | Postgres role `historiador_mcp` has SELECT-only grants on whitelisted tables; MCP binary reads `DATABASE_URL_READONLY`. **DB-layer enforcement is correct.** |
| Split-pane AI editor (conversation + preview) | 🟡 Partial | Editor has a chat-style SSE stream, not the WebSocket-based conversational↔generation mode switching described in ADR-008. No section click-to-edit. No auto-save (client-side only). |
| Multilingual editor (pre-publish check) | ✅ Complete | Warns on missing languages but does not block publish — matches PRD (explicit non-blocking requirement). |
| Page version history + restore-as-draft | ✅ Complete | Fully implemented despite README still claiming "Sem historico de versao de paginas" (stale). |
| Full-workspace markdown export (ZIP) | ✅ Complete | Sprint 8 delivered via `async_zip`. |
| Ollama local LLM support | ✅ Complete | Ollama is a first-class provider; embedding stub exists for providers without native embedding support. |
| First-run setup wizard (423 Locked gate) | ✅ Complete | Correctly gates all routes except `/health`, `/setup/init`, `/setup/probe`, `/docs/`. |
| RBAC (Admin/Author/Viewer) | 🟡 Partial | Roles enforced at use-case layer. See finding 5.2 — this works today but is not defense-in-depth. |
| Argon2id password hashing | ✅ Complete | Uses `argon2` crate with recommended params. |
| JWT HS256 with ≥32-char secret | ✅ Complete | Boot-time validation crashes if secret is too short. |
| MCP bearer token — timing-safe validation | ❌ Not complete | Uses `==` on `&[u8]`. See finding 4.1 — **this is a P0 security defect per Sprint 9 DoD.** |
| Load test: 1k queries @ 10k chunks, p95 < 2s | ❌ Not done | No `docs/performance.md`, no load test script committed, no performance numbers recorded. |
| CI: fmt + clippy (deny warnings) + test + audit | ✅ Complete | Three parallel jobs; OpenAPI drift gate also present. |
| `CONTRIBUTING.md` | ❌ Missing | Sprint 9 DoD item; not present in repo. |
| `CHANGELOG.md` (Keep-a-Changelog) | ❌ Missing | Sprint 9 DoD item; not present in repo. |
| Public repo + `v1.0.0` tag + GitHub Release | ❌ Not done | No tags exist (`git tag -l` → empty). |
| ≥5 `good first issue` items filed | ❌ Not done | Can't verify without GitHub API access, but no evidence of release-prep activity. |

**Legend:** ✅ complete / 🟡 partial / ❌ missing.

**Coverage:** 15 ✅ / 5 🟡 / 6 ❌ of the 26 P0 items listed. Completion in working-feature terms is ~77% of scope and ~60% of release-readiness.

---

## 3. Sprint-by-Sprint Landing

| Sprint | Goal | Landed? | Notable gaps |
|---|---|---|---|
| 1 | Skeleton + migrations + `/health` | ✅ | — |
| 2 | Auth, setup wizard, LLM provider config | ✅ | Setup wrapped in DB transaction (✓). |
| 3 | Pages CRUD, collections, chunker v1 | ✅ | Chunker metadata missing `author_id`/`last_updated` — see 5.3. |
| 4 | LLM abstraction + AI editor | 🟡 | Editor shipped with SSE instead of WebSocket per ADR-008; no conversation/generation mode toggle. |
| 5 | MCP server + bearer auth | 🟡 | Custom REST endpoint instead of MCP JSON-RPC 2.0; token comparison not timing-safe. |
| 6 | Multilingual + OpenAPI pipeline | ✅ | — |
| 7 | Version history + analytics | ✅ | Full analytics dashboard + Chronik `mcp-queries` topic (or local fallback). |
| 8 | Export + Ollama | ✅ | — |
| 9 | Hardening + public release | ❌ | Abandoned after Monday. No load test, no release artifacts, no tag. |

---

## 4. Critical Issues (must fix before v1.0.0)

### 4.1 🔴 MCP bearer token is not timing-safe

**File:** `apps/mcp/src/auth.rs`

The bearer token is compared with `==` on two `&[u8]` slices. Rust's `PartialEq` for slices short-circuits on the first mismatched byte, so the comparison leaks token length and prefix through wall-clock timing. The PRD's security invariant and the Sprint 9 DoD item ("confirm timing-safe comparison") both require a constant-time comparison here.

**Severity:** Critical — this is the single externally-exposed surface of the entire system. The MCP port (3002) is the only port the PRD authorizes exposing to the internet.

**Fix:** swap to `subtle::ConstantTimeEq` (already a transitive dep via `aes-gcm`), or add `constant_time_eq = "0.3"` and use `constant_time_eq::constant_time_eq(provided, expected)`. Both tokens must be hashed (SHA-256) and the comparison performed on the fixed-length digest to avoid also leaking length.

### 4.2 🔴 MCP endpoint does not speak the MCP protocol

**Files:** `apps/mcp/src/handlers.rs`, `apps/mcp/src/routes.rs`

The README and PRD promise "every knowledge base ships with a built-in MCP server" and show Claude Desktop being configured with `{ "url": "...", "token": "..." }`. A real MCP server must:
- Speak JSON-RPC 2.0 over HTTP (or stdio) per the [MCP spec](https://modelcontextprotocol.io/)
- Implement `initialize`, `tools/list`, `tools/call`, and ideally `resources/list`/`resources/read`
- Advertise a `query` tool (or similar) via `tools/list` that Claude Desktop can discover

The current handler is `POST /query` returning a bespoke JSON shape. Claude Desktop cannot connect to it. The README's configuration snippet is actively misleading.

**Severity:** Critical — this is the defining feature of the product ("MCP-native since day one"). If Claude Desktop cannot connect, the product does not do what it says.

**Fix:** add a JSON-RPC 2.0 dispatcher in front of the existing query logic. The `rmcp` or `mcp-server` crates (or hand-rolled JSON-RPC routing) can expose the same underlying query function as an MCP tool. The existing handler can remain as an "internal" endpoint for the web UI.

### 4.3 🔴 Editor does not implement the ADR-008 split-pane contract

**Files:** `apps/web/components/editor/editor-panel.tsx`, `apps/web/features/editor/use-editor-stream.ts`

ADR-008 specifies a WebSocket connection with two explicit modes — **conversation mode** (chat with the author) and **generation mode** (author clicks a section, Claude rewrites it inline). The current implementation:
- Uses Server-Sent Events (SSE) instead of WebSocket (one-way; cannot fulfill ADR-008's bidirectional contract)
- Has no mode toggle — only a single chat stream
- Has no section click-to-edit in the preview pane
- Keeps conversation history in component state (lost on refresh/route change)
- Has no auto-save of draft content; unsaved changes die with the tab

**Severity:** Critical for UX; medium for data integrity (no auto-save is a bug). The SSE approach is a valid v0.9 placeholder but does not match the PRD promise.

**Fix options (prioritize by sprint budget):**
- **Minimum** to ship v1.0: add auto-save (debounced 2s save-as-draft) and persist conversation to the DB. Defer WebSocket/mode-toggle to v1.1 and document the deviation in `CHANGELOG.md`.
- **Complete**: rebuild editor transport on WebSocket with explicit `mode` frames and inline section editing.

### 4.4 🟠 Chronik integration is optional with a silent in-memory fallback

**Files:** `apps/api/src/infrastructure/vector_store/*`, README `Limitacoes conhecidas`

If Chronik is unreachable at startup, the API falls back to an in-memory vector store. This means chunks "persist" until the container restarts, at which point they vanish. The README still advertises this as a known limitation — but since Sprint 7 added page version history (which assumes durable chunk storage), this fallback now silently breaks a feature the product claims.

**Severity:** High — operators who skim the README may deploy without Chronik and lose retrieval on every restart without an error surface.

**Fix:** make Chronik configuration mandatory in production mode. Gate it behind an env var (`ALLOW_IN_MEMORY_VECTOR_STORE=true` for dev only). The API should `anyhow::bail!` at boot if the env is production and Chronik is unreachable.

### 4.5 🔴 Sprint 9 release artifacts do not exist

Missing, all required by Sprint 9 DoD:
- `CONTRIBUTING.md`
- `CHANGELOG.md` (Keep-a-Changelog format)
- `docs/performance.md` (load test report)
- `v1.0.0` git tag
- GitHub Release
- ≥5 `good first issue` tickets
- `cargo audit` pass confirmation (CI runs it, but no release-time snapshot exists)

**Severity:** Critical for public launch; the repo is not ready to be linked to publicly.

**Fix:** dedicated release hardening sprint. Details in Section 7.

---

## 5. High-Impact Findings (fix before or immediately after v1.0)

### 5.1 🟠 README contains stale limitations

**File:** `README.md` lines 159–166

The README documents three "known limitations" that were resolved in Sprints 7–8:
- "Sem historico de versao de paginas" — **false**, version history landed in Sprint 7
- "Sem suporte a embeddings via Ollama" — **partially false**, Ollama provider exists; only native Ollama embedding is stubbed
- "VexFS integration in progress" — **misleading**, ADR-007 replaced VexFS with Chronik-Stream

This directly contradicts features the product now has. Update as part of the CHANGELOG/release pass.

### 5.2 🟠 RBAC is enforced at the use-case layer, not at route middleware

**Files:** `apps/api/src/presentation/routes/*`, `apps/api/src/application/*`

Every use case checks `role == Admin` or `role == Author` before executing. This works today and every endpoint is covered. The risk is **defense-in-depth**: any future contributor adding a new use case can forget the check, and there is no route-level middleware to catch the miss. A role-checking `axum::middleware::from_fn` attached to route groups (`/admin/*` → Admin, `/pages/*` → Author) would add a second layer without removing the existing checks.

**Severity:** Medium. Not a bug in v1.0; a reliability concern for v1.1.

### 5.3 🟠 Chunker emits incomplete metadata

**File:** `crates/chunker/src/lib.rs`

Chunks carry `page_id`, `language`, `heading_path`, and `position` but are missing:
- `author_id` — needed for per-author analytics promised in the PRD
- `last_updated` — needed for freshness-based ranking hints

Both are already on `page_versions`, so the chunker just needs to receive them in its input and propagate them.

**Severity:** Medium. Features that depend on them (e.g., author analytics in the admin dashboard) will be incomplete until this lands.

### 5.4 🟠 No production docker-compose profile

**File:** `docker-compose.yml`

The existing compose file starts Postgres and Ollama for local development. The PRD's acceptance criterion is "runs via Docker Compose on a standard Linux VPS (2 vCPU / 4 GB minimum)." For that to be true, there needs to be a `docker-compose.prod.yml` (or a profile) that:
- Uses pinned image tags rather than `:latest`
- Runs API and MCP as containers (currently they run on the host via `cargo run`)
- Sets `restart: unless-stopped` on all services
- Binds only the MCP port (3002) to `0.0.0.0`; all others to `127.0.0.1`
- Ships with a sample `nginx.conf` or Caddy config for TLS termination

**Severity:** Medium. Without this, the "self-hosted on a VPS" promise has a non-trivial DIY gap.

### 5.5 🟡 `scripts/setup-vexfs.sh` still present

VexFS was superseded by Chronik-Stream per ADR-007. The vendoring script is dead code and confusingly contradicts the current architecture.

**Severity:** Low. Delete or move to `scripts/archive/` with a header explaining it's historical.

---

## 6. What Actually Works Well

It's worth documenting what the codebase does right so the hardening sprint doesn't accidentally regress it:

- **Clean/hexagonal architecture in the API** — domain/application/infrastructure/presentation is cleaner than the PRD asked for and is a net positive for maintainability.
- **OpenAPI codegen pipeline is airtight** — `utoipa` annotations → `openapi.yaml` → TypeScript types, with a CI drift gate. This was one of the PRD's explicit invariants and it landed cleanly.
- **Chunker is structure-aware, not fixed-size.** The comrak AST traversal respects heading boundaries, and code blocks/tables/lists are atomic per ADR-002.
- **Multilingual ADR-005 is honored everywhere**: the `page_versions` composite key, the editor tabs, the chunker's `language` field, and the MCP query filter.
- **DB-layer MCP isolation is correct.** `historiador_api` and `historiador_mcp` Postgres roles are created by an init script, and the MCP binary uses `DATABASE_URL_READONLY`. The zero-write invariant is enforced at the database level, not just in application code — exactly what ADR-003 requires.
- **CI is strict.** `cargo clippy -D warnings`, `cargo fmt --check`, `cargo audit`, and an OpenAPI drift gate all run on every PR.
- **Setup wizard is wrapped in a DB transaction** — a subtle thing the PRD called out explicitly and that landed correctly.

---

## 7. Recommended Path to v1.0.0

The remaining work is almost exactly the Sprint 9 backlog, with three feature-shaped additions lifted from above. I'd treat it as one re-scoped hardening sprint rather than a fresh v2 plan.

### 7.1 Re-scoped Sprint 9 — "v1.0 Hardening" (5–7 working days)

**Day 1 — Security & contract fixes (unblockers)**
- Replace `==` bearer comparison with `constant_time_eq` on a SHA-256 digest (4.1)
- Decide MCP protocol strategy: (a) JSON-RPC 2.0 wrapper added now and ship v1.0 MCP-compliant, or (b) rename current endpoint to `/query` (internal) and ship v1.0 with "MCP endpoint: coming in v1.1" in CHANGELOG. **Strongly recommend (a)** — the whole product thesis is MCP-native. (4.2)
- Run `cargo audit`; address any advisories

**Day 2 — Chronik & production readiness**
- Gate the in-memory vector fallback behind `ALLOW_IN_MEMORY_VECTOR_STORE=true` (4.4)
- Add `docker-compose.prod.yml` with pinned tags, restart policies, and correct port binding (5.4)
- Delete or archive `scripts/setup-vexfs.sh` (5.5)

**Day 3 — Editor minimum bar**
- Add debounced draft auto-save (every 2s of inactivity, 30s maximum)
- Persist editor conversation to a new `editor_conversations` table keyed by `(page_id, language, user_id)`
- Document the SSE→WebSocket migration as a v1.1 item

**Day 4 — Load test & performance doc**
- Seed 10,000 chunks in a test workspace (script goes in `scripts/load-test/seed.rs`)
- Run 1,000 MCP queries with `oha` or `k6`
- Record p50/p95, commit to `docs/performance.md`
- If p95 > 2s, tune Chronik HNSW `ef_search`/`m` or add query-embedding cache (60s TTL)

**Day 5 — Release artifacts**
- Write `CONTRIBUTING.md` (local setup + tests + OpenAPI pipeline + PR conventions)
- Write `CHANGELOG.md` with complete v1.0.0 entry and honest "Known limitations"
- Update README: delete stale limitations (5.1), add "MCP protocol compliance" note, link to `docs/performance.md`
- File ≥5 `good first issue` tickets (e.g., "Add pt-PT to supported languages list", "Improve error message when JWT_SECRET is too short", "Dark mode for dashboard", "Replace `scripts/setup-vexfs.sh` with a migration guide", "Add French translations to the setup wizard strings")
- Tag `v1.0.0`, publish GitHub Release, make the repo public

### 7.2 Deferred to v1.1 (file as GitHub issues before tagging v1.0.0)

- Rebuild editor on WebSocket with ADR-008 conversation/generation mode switching and inline section click-to-edit (4.3 — complete fix)
- Route-level RBAC middleware as defense-in-depth (5.2)
- Chunker metadata: `author_id` + `last_updated` propagation (5.3)
- Email delivery for invitations (currently manual link sharing)
- Native Ollama embeddings (currently stubbed)

### 7.3 Deferred to v2 (already captured in `v2-plan.md`)

Gap detection flywheel, SSO/SAML, multi-workspace, webhooks, public docs site. These are correctly gated on "2–4 weeks of real usage data from the Chronik `mcp-queries` topic" and are not blockers for v1.0.0.

---

## 8. Risk Register for the Hardening Sprint

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| MCP JSON-RPC migration breaks existing web-UI calls | Medium | Medium | Keep the current `POST /query` as internal endpoint; add JSON-RPC 2.0 as a parallel route. Point Claude Desktop at the new route in the README. |
| Load test reveals p95 > 2s with no obvious fix | Low | High | Already budgeted 2pts for tuning in Sprint 9. If tuning fails, document actual p95 in `docs/performance.md` and ship — do not slip the release. |
| `cargo audit` finds a critical advisory on a pinned dep | Low | Medium | Run audit on Day 1, not Day 5. |
| Timing-safe comparison crate pulls a transitive `subtle` version mismatch with `aes-gcm` | Low | Low | `aes-gcm` already depends on `subtle`; reuse it directly. |
| ADR-008 is outdated and SSE was an informed re-scope, not an oversight | Medium | Low (reframes 4.3) | Before treating 4.3 as a defect, confirm with the author whether ADR-008 still represents the product intent. If not, write an ADR-009 that formally supersedes it. |

---

## 9. Suggested Immediate Actions (today)

If the next working session is short, prioritize in this order:

1. **Fix 4.1** (timing-safe comparison) — one-hour change, closes a P0 security hole
2. **Fix 4.4** (gate in-memory fallback) — one-hour change, prevents silent data loss in production
3. **Update README stale limitations** (5.1) — 15 minutes, removes misleading claims
4. **Open a GitHub milestone for "v1.0.0 release"** and move the rest of Section 7.1 into it as issues
5. **Decide on ADR-008 status** — reaffirm or write ADR-009 superseding it. The answer determines how much editor work is in scope for v1.0 vs. v1.1.

---

## 10. Verdict

**Request changes.** The codebase is closer to v1.0 than its release artifacts suggest — the features listed in the PRD are substantially built and the architecture is clean. But two PRD contract promises (MCP protocol, editor transport) are not honored in the implementation, one security primitive fails its own PRD acceptance criterion, and none of the release hardening Sprint 9 expected has been done.

A focused 5-day hardening sprint along the lines of Section 7.1 should land a defensible `v1.0.0`. Going public without it risks (a) Claude Desktop users finding the MCP endpoint doesn't speak MCP, and (b) a third party running a timing-attack proof-of-concept on the bearer token and filing a public issue before the ink is dry on the release.
