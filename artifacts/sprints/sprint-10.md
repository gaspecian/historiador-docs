# Sprint 10 — v1.0 Hardening & Public Release (retry)

**Dates:** Weeks 10–11 (10 working days)
**Team:** Gabriel Specian (solo)
**Sprint Goal:** Close every P0 gap from the [v1.0 code review](code-review-v1.0-readiness.md), tag `v1.0.0`, and make the GitHub repository public — with a real MCP-protocol endpoint, timing-safe auth, durable vector storage, an auto-saving editor, and the release artifacts that Sprint 9 never produced.

---

## Context

Sprint 9 was the original release sprint but stopped after Monday — only the `sprint 9 monday` commit exists and no `v1.0.0` tag was cut. The code review completed on 2026-04-18 found that the feature work from Sprints 1–8 is substantially there, but four categories of gaps prevent a responsible public launch:

1. Two PRD contract promises are implemented the wrong way (editor uses SSE instead of WebSocket per ADR-008; MCP endpoint is custom REST instead of JSON-RPC 2.0 per the MCP spec).
2. The MCP bearer token comparison is not timing-safe, which breaks the Sprint 9 DoD and the PRD's security invariant for the one externally-exposed port.
3. Chronik is optional with a silent in-memory fallback — version history relies on durability that this fallback breaks.
4. No release artifacts exist: no `CONTRIBUTING.md`, no `CHANGELOG.md`, no `docs/performance.md`, no tag, no Release.

This sprint re-scopes Sprint 9 with the extra items the review surfaced. Two weeks (16 pts) instead of one because adding the MCP JSON-RPC wrapper and editor auto-save was not in the original Sprint 9 plan. Everything deferred to v1.1 (full editor WebSocket rebuild, route-level RBAC middleware, chunker metadata expansion, email delivery, native Ollama embeddings) is explicitly out of scope — log it as GitHub issues before tagging.

---

## Capacity

| Person | Available Days | Allocation | Notes |
|--------|---------------|------------|-------|
| Gabriel | 10 of 10 | 16 pts committed / 4 stretch | 2-week release sprint — no scope expansion |
| **Total** | **10** | **16 pts** | 1 point ≈ ~half a day |

---

## Sprint Backlog — P0

### Week 1 — Security & contract fixes (8 pts)

| # | Item | Points | Notes |
|---|------|--------|-------|
| 1 | **Timing-safe MCP bearer token comparison** | 1 pt | Replace `==` on `&[u8]` in `apps/mcp/src/auth.rs` with `constant_time_eq::constant_time_eq` on a SHA-256 digest of the provided token vs. the stored digest. Hash the stored token at setup time so the comparison is always fixed-length. Add a regression test that asserts the comparison returns false for a prefix match (current implementation passes this by accident). Closes finding 4.1 of the code review. |
| 2 | **MCP JSON-RPC 2.0 wrapper** | 3 pts | Add a JSON-RPC 2.0 dispatcher at `POST /mcp` that implements `initialize`, `tools/list`, `tools/call`, and optionally `resources/list`. Register a `query` tool that invokes the existing query logic. Keep the current `POST /query` as an internal alias so the web UI continues to work. Update the README's Claude Desktop config to point at `/mcp`. Validate against the MCP spec sample client. Closes finding 4.2. |
| 3 | **Gate in-memory vector fallback** | 1 pt | Add `ALLOW_IN_MEMORY_VECTOR_STORE` env var (default `false`). At boot, if Chronik is unreachable and the flag is false, `anyhow::bail!` with a message telling the operator to start Chronik or set the flag for dev only. Log a loud warning on every API startup when the flag is enabled. Closes finding 4.4. |
| 4 | **Editor auto-save + conversation persistence** | 2 pts | Debounced draft auto-save every 2s of inactivity, 30s max. New `editor_conversations` table keyed by `(page_id, language, user_id)` persists the chat history so a page refresh or route change doesn't lose context. Does **not** rebuild the editor transport on WebSocket — that's deferred to v1.1 per the code review's recommendation. Add a banner in the editor reading "Saved <timestamp>" or "Unsaved changes…" so authors can see the state. Closes the minimum bar of finding 4.3. |
| 5 | **Security audit + `cargo audit`** | 1 pt | Run `cargo audit` and address any advisories found. Confirm (a) bearer validation path hits item #1's new code; (b) every non-public API route requires auth (grep for handlers that don't take `AuthUser`); (c) Postgres and Chronik bind to the Docker network only, never `0.0.0.0`; (d) no secrets in `.env.example` or test fixtures. Document findings in `docs/security.md`. |

### Week 2 — Production readiness & release (8 pts)

| # | Item | Points | Notes |
|---|------|--------|-------|
| 6 | **Production docker-compose profile** | 1 pt | New `docker-compose.prod.yml`. Pinned image tags (no `:latest`). `restart: unless-stopped` on all services. API and MCP run as containers (not `cargo run` on host). Only MCP port (3002) binds `0.0.0.0`; everything else `127.0.0.1`. Include a sample `nginx.conf` for TLS termination in `docs/deploy/`. Closes finding 5.4. |
| 7 | **Load test — MCP @ 10k chunks** | 1 pt | Seed 10,000 chunks via a new `scripts/load-test/seed.rs`. Run 1,000 MCP queries through the new JSON-RPC endpoint using `oha`. Record p50 and p95. Commit to `docs/performance.md`. Target: p95 < 2s. Carried over verbatim from Sprint 9 item #1. |
| 8 | **Performance tuning (conditional)** | 2 pts | **Only execute if #7 p95 > 2s.** Levers in priority order: (a) Chronik HNSW `ef_search`/`m` for the `published-pages` topic; (b) Axum connection pool size in `crates/mcp`; (c) 60s TTL query-embedding cache for identical query strings; (d) `cargo flamegraph` to find the actual bottleneck. Document all tuning decisions in `docs/performance.md`. Carried over from Sprint 9 item #2. |
| 9 | **Docker image optimization** | 1 pt | Verify `cargo-chef` correctly caches dependency layers for both `api` and `mcp` binaries. Confirm final binary size < 50MB uncompressed. Remove any dev-only tools (`cargo-watch`, etc.) from the production image. Multi-stage build confirmed. Also: delete or move `scripts/setup-vexfs.sh` to `scripts/archive/` with a README explaining VexFS was superseded by Chronik per ADR-007. Closes Sprint 9 item #3 + finding 5.5. |
| 10 | **`CONTRIBUTING.md`** | 1 pt | Local dev setup (Rust toolchain version, pnpm version, Docker), how to run tests, how the OpenAPI pipeline works, PR conventions, issue labels. Target: a Rust developer can make their first contribution in under 2 hours of reading. Carried over from Sprint 9 item #5. |
| 11 | **`CHANGELOG.md` + README cleanup** | 1 pt | `CHANGELOG.md` in Keep-a-Changelog format with a complete `v1.0.0` entry. Sections: Added (all P0 features), Changed (MCP endpoint moved to `/mcp`), Known Limitations (editor still SSE-based; email delivery manual; native Ollama embeddings stubbed). Update the README: remove the three stale limitations (finding 5.1), add an MCP protocol compliance section, link to `docs/performance.md` and `docs/security.md`. Carried over from Sprint 9 item #6, plus finding 5.1. |
| 12 | **Public GitHub release (`v1.0.0`)** | 1 pt | Run `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test --workspace`, `pnpm lint` one last time. Tag `v1.0.0`. Make the repo public. Publish GitHub Release with notes derived from `CHANGELOG.md`. Create labels (`bug`, `enhancement`, `good first issue`, `documentation`, `performance`, `multilingual`, `v1.1`, `v2`). File ≥5 `good first issue` items: add pt-PT to supported languages, improve `JWT_SECRET too short` error message, dark mode for dashboard, replace `scripts/setup-vexfs.sh` with a migration guide, add French to setup wizard strings. Also file the 5 v1.1 deferrals (finding 4.3 full rebuild, 5.2 route RBAC, 5.3 chunker metadata, email, native Ollama embeddings). Closes Sprint 9 item #7. |

**Planned: 16 pts (100% of capacity).** If #8 (perf tuning) is not needed, 2 pts reclaim for stretch.

---

## Stretch (4 pts — or reclaimed if load test passes first try)

| Item | Points | Notes |
|------|--------|-------|
| Route-level RBAC middleware | 2 pts | Defense-in-depth: `axum::middleware::from_fn` groups for `/admin/*` (Admin), `/pages/*` (Author+), `/search/*` (Viewer+). Does not remove existing use-case-layer checks. Closes finding 5.2. |
| Chunker metadata: `author_id` + `last_updated` | 1 pt | Propagate from `page_versions` through the chunker and into chunk payloads. Closes finding 5.3. |
| Installation screencast | 1 pt | 5-minute screencast: `docker compose up` → setup wizard → write a page → query via Claude Desktop through the new `/mcp` endpoint. Embed in README. Carried over from Sprint 9 stretch. |

---

## Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| MCP JSON-RPC migration is larger than 3 pts because the MCP spec is ambiguous on auth headers | Slips the release by 2–3 days | Pre-read the spec (Day 0, over the weekend before sprint start). Scope explicitly to `initialize`, `tools/list`, `tools/call` — skip `resources/*` if time-pressed and file it as a v1.1 issue. The web UI keeps using the existing `/query` endpoint so nothing regresses. |
| Load test reveals p95 > 2s and tuning (2 pts) can't close it | Release blocks on perf | Same mitigation as Sprint 9: document actual p95 in `docs/performance.md` with a remediation plan and ship anyway if the delta is modest (<3s). Do not slip the release on a number that may need Chronik-level changes beyond this sprint's scope. |
| `cargo audit` surfaces a critical advisory on a deep dependency | Release blocks on dep upgrade | Run audit on Day 1, not Day 10. Keeps 9 days to upgrade and re-test. |
| Editor auto-save introduces a race condition between client debounce and server writes | Data loss | Use optimistic concurrency: include `version_id` in the save request; server 409s on conflict and the client reloads. Already have `page_versions` table so the primitive is there. |
| ADR-008 was actually an informed re-scope and SSE is the right transport | 4.3 is not a real defect | Reaffirm ADR-008 or write ADR-009 superseding it on **Day 1 before writing any code**. If superseded, this item becomes 0 pts (still do auto-save but not conversation persistence) and 2 pts free up for stretch. |
| Chronik durability gate breaks someone's dev setup | Loud complaints on Day 1 of public launch | Default the env flag to `true` in `.env.example` (dev-friendly). Default to `false` in `docker-compose.prod.yml`. Document the split clearly in `CHANGELOG.md` under "Changed". |

---

## Definition of Done

### Security & contract (Week 1 gates)
- [ ] MCP bearer comparison uses `constant_time_eq` on a SHA-256 digest, with a regression test covering prefix-match timing
- [ ] JSON-RPC 2.0 endpoint at `POST /mcp` implements `initialize`, `tools/list`, `tools/call`; validated against a real MCP client (Claude Desktop or the MCP spec's sample client)
- [ ] `ALLOW_IN_MEMORY_VECTOR_STORE` env var gates the fallback; API fails boot in production mode if Chronik is down
- [ ] Editor auto-saves drafts (debounced) and persists conversation to `editor_conversations`; tested across page refresh and route change
- [ ] `cargo audit` passes with no critical advisories; findings logged in `docs/security.md`

### Production & performance (Week 2 gates)
- [ ] `docker-compose.prod.yml` exists, uses pinned tags, restart policies, and binds only MCP port externally
- [ ] Load test report in `docs/performance.md`: p50 and p95 for 1,000 queries against 10,000 chunks
- [ ] p95 < 2s, OR deviation documented with remediation plan
- [ ] Rust binaries < 50MB each; `cargo-chef` caching verified
- [ ] `scripts/setup-vexfs.sh` archived

### Release artifacts
- [ ] `CONTRIBUTING.md` covers setup, tests, OpenAPI pipeline, PR conventions
- [ ] `CHANGELOG.md` has complete `v1.0.0` entry in Keep-a-Changelog format
- [ ] README's three stale limitations removed; MCP compliance section added; performance + security doc links added
- [ ] `cargo fmt --check`, `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`, `pnpm lint` all green on the release commit
- [ ] Repository is public on GitHub
- [ ] `v1.0.0` tag and GitHub Release published
- [ ] ≥5 `good first issue` tickets filed
- [ ] 5 v1.1 deferrals filed as issues and added to a `v1.1.0` milestone
- [ ] v2 items from [v2-plan.md](v2-plan.md) filed as issues and added to a `v2.0.0` milestone

---

## Key Dates

| Date | Event |
|------|-------|
| Week 10 Monday | Sprint start — ADR-008 reaffirm-or-supersede decision; `cargo audit`; start on MCP JSON-RPC wrapper |
| Week 10 Tuesday | Timing-safe bearer fix + MCP JSON-RPC (continued) |
| Week 10 Wednesday | Chronik durability gate; MCP JSON-RPC wrap-up; validate against Claude Desktop |
| Week 10 Thursday | Editor auto-save + conversation persistence |
| Week 10 Friday | Security audit; mid-sprint check-in; Week 1 items merged |
| Week 11 Monday | Production docker-compose profile + Docker image optimization + `setup-vexfs.sh` archival |
| Week 11 Tuesday | Seed 10k chunks; run load test; document results |
| Week 11 Wednesday | Performance tuning (if needed); otherwise stretch items |
| Week 11 Thursday | `CONTRIBUTING.md` + `CHANGELOG.md` + README cleanup |
| Week 11 Friday EOD | File v1.1 + v2 issues; tag `v1.0.0`; publish GitHub Release; make repo public 🎉 |

---

## Backlog for v1.1 (file as issues before tagging)

These are the deferrals from the code review that this sprint explicitly does NOT cover:

| Item | Source | Estimate (v1.1) |
|------|--------|-----------------|
| Rebuild editor on WebSocket per ADR-008 with conversation↔generation mode switching and inline section edit | Review 4.3 (complete fix) | 3 pts |
| Route-level RBAC middleware | Review 5.2 | 2 pts |
| Chunker: propagate `author_id` + `last_updated` metadata | Review 5.3 | 1 pt |
| Email delivery for invitations | Alpha limitation | 2 pts |
| Native Ollama embeddings (remove stub) | Alpha limitation | 2 pts |

---

## v1.0.0 Release Narrative (draft for CHANGELOG)

```markdown
## [1.0.0] — 2026-05-01

### Added
- Self-hosted documentation platform with built-in MCP server (JSON-RPC 2.0 at `POST /mcp`)
- Dual representation: markdown authoring ↔ structure-aware chunks in Chronik-Stream
- Multilingual by default (BCP-47 language config, per-language page versions)
- Split-pane editor with conversation-mode LLM authoring, auto-save, and persisted chat history
- Page version history with restore-as-draft
- Full-workspace markdown export (ZIP)
- First-run setup wizard; 423 Locked gate until complete
- RBAC with Admin/Author/Viewer roles
- Ollama + OpenAI + Anthropic LLM providers
- Analytics dashboard with MCP query tracking

### Security
- Argon2id password hashing
- JWT HS256 with mandatory ≥32-char secret
- Timing-safe MCP bearer token validation
- Postgres role separation: `historiador_api` (RW), `historiador_mcp` (SELECT-only on whitelisted tables)

### Known Limitations
- Editor transport is Server-Sent Events; WebSocket with inline section-edit mode ships in v1.1
- Invitation emails must be shared manually; native email delivery in v1.1
- Native Ollama embeddings are stubbed (OpenAI/Anthropic embeddings work natively)

### What's Next (v2)
- Gap detection flywheel: MCP queries with zero results cluster into documentation requests
- SSO/SAML (Okta, Azure AD, Google Workspace)
- Multi-workspace support
- Public documentation site with custom domains
```
