# Sprint 9 — Performance Validation + Public Open-Source Release (v1.0.0)

**Dates:** Week 9 (5 working days)
**Team:** Gabriel Specian (solo)
**Sprint Goal:** All P0 requirements are met and validated, the MCP server hits the p95 < 2s latency target at realistic scale, and `v1.0.0` is tagged and publicly released on GitHub with documentation that lets any developer complete a full installation in under 30 minutes.

---

## Context

This is the final sprint of the v1.0 cycle. The work is not feature work — it's validation, hardening, and public launch preparation. The sprint has three independent threads:

1. **Performance validation** — prove the system meets its own success metrics under realistic load
2. **Release preparation** — `CONTRIBUTING.md`, `CHANGELOG.md`, security hardening, Docker image optimization
3. **Public release** — the GitHub repository goes public; `v1.0.0` is tagged

No new features ship in this sprint. Any P1 item that did not land in Sprints 6–8 is explicitly deferred to v1.1 and logged in the GitHub issue tracker before EOD Friday.

---

## Capacity

| Person | Available Days | Allocation | Notes |
|--------|---------------|------------|-------|
| Gabriel | 5 of 5 | 8 pts committed / 2 stretch | Release sprint — no scope expansion |
| **Total** | **5** | **8 pts** | 1 point ≈ ~half a day |

---

## Sprint Backlog — P0

| # | Item | Points | Notes |
|---|------|--------|-------|
| 1 | **Load test — MCP server at 10k chunks** | 1 pt | Seed a test workspace with 10,000 chunks (representative of a mid-sized company knowledge base). Run 1,000 MCP queries against the live stack. Measure and record p50 and p95 latency. Target: p95 < 2s. Use `oha` or `k6` for load generation. If p95 > 2s, trigger the performance tuning item (see below). |
| 2 | **Performance tuning (conditional)** | 2 pts | **Only execute if load test p95 > 2s.** Tuning levers in priority order: (a) Chronik HNSW index parameters — adjust `ef_search` and `m` for the `published-pages` topic; (b) Axum connection pool size in `crates/mcp`; (c) embedding caching — cache the query embedding for identical query strings with a short TTL (60 seconds) to avoid redundant embedding API calls; (d) if still over target, profile with `cargo flamegraph` and address the actual bottleneck. Document the tuning decisions in a `docs/performance.md` file. |
| 3 | **Docker image optimization** | 1 pt | Confirm `cargo-chef` is correctly configured in the Dockerfile for both `api` and `mcp` binaries — dependency layers should be cached separately from application code. Verify final binary sizes are reasonable (target: each Rust service < 50MB uncompressed). Remove any development-only tools (`cargo-watch`, etc.) from the production image. Multi-stage build confirmed: builder stage + minimal runtime stage. |
| 4 | **Security hardening** | 1 pt | Audit the three highest-risk surfaces: (a) bearer token validation on the MCP endpoint — confirm timing-safe comparison; (b) confirm all API endpoints require auth except `GET /health` and the setup wizard; (c) confirm Chronik and PostgreSQL are not exposed outside the Docker network (only the MCP server port 3002 is exposed externally). Run `cargo audit` — address any dependency advisories. |
| 5 | **`CONTRIBUTING.md` + developer setup** | 1 pt | Write `CONTRIBUTING.md`: local development setup (Rust toolchain version, pnpm version, Docker), how to run tests (`cargo test`, `pnpm test`), how the OpenAPI pipeline works, PR conventions, issue labels. Target: a developer with Rust experience can make their first contribution in under 2 hours from reading this file. |
| 6 | **`CHANGELOG.md` + release notes** | 1 pt | Write `CHANGELOG.md` for `v1.0.0`. Format: Keep a Changelog (`https://keepachangelog.com`). Sections: Added (all P0 features), Known Limitations (Chronik stub if not fully integrated, any known issues), What's Next (v2 gap detection flywheel). Also write the GitHub Release description — this is the public-facing narrative for the open-source community. |
| 7 | **Public GitHub release (`v1.0.0`)** | 1 pt | Make the repository public. Tag `v1.0.0`. Publish the GitHub Release with the release notes from `CHANGELOG.md`. Create the following GitHub labels: `bug`, `enhancement`, `good first issue`, `documentation`, `performance`, `multilingual`. File at least 5 `good first issue` issues covering approachable contributions (e.g., adding a new language to the supported list, improving error messages, adding a dark mode to the dashboard). |

**Planned: 8 pts (80% capacity)** *(Performance tuning is conditional — if load test passes, 2 pts free up for stretch)*

---

## Stretch (2 pts — or reclaimed if load test passes)

| Item | Points | Notes |
|------|--------|-------|
| Load test with Ollama | 1 pt | Repeat the load test with Ollama configured as the LLM provider instead of OpenAI. Document p95 latency. Local models are typically slower — this establishes a baseline expectation for Ollama users. |
| Installation screencast | 1 pt | Record a 5-minute screencast: `docker compose up` → setup wizard → write a page → query via Claude Desktop. Embed in the README. Significantly lowers the barrier for evaluators who won't read documentation. |

---

## Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| Load test fails p95 target | Release is blocked until the target is met | Budget 2 pts for performance tuning. If tuning cannot meet the target within the sprint, document the current p95 in `docs/performance.md` and defer the target — do not delay the public release over a performance optimization that may require Chronik-level changes beyond the sprint's scope. |
| Unresolved P1 items from earlier sprints | If page version history, Ollama, or export are incomplete, they create pressure to slip the release | Create a `v1.1.0` milestone on GitHub on Monday. Move any incomplete P1 items there explicitly. The release criterion is P0 completeness — not P1 completeness. |
| `cargo audit` finds critical advisories | A critical dependency vulnerability blocks a clean release | Run `cargo audit` on Monday, not Friday. Gives 4 days to update dependencies and re-run tests. |

---

## Definition of Done

- [ ] Load test report: 1,000 queries against 10,000 chunks, p50 and p95 recorded, stored in `docs/performance.md`
- [ ] p95 MCP query latency is < 2s, OR the deviation is documented with a remediation plan
- [ ] `cargo audit` returns no critical advisories
- [ ] Chronik and PostgreSQL are not reachable outside the Docker network; only port 3002 is exposed externally
- [ ] All API endpoints require authentication except `GET /health` and the first-run setup wizard
- [ ] Docker images use multi-stage builds; Rust service binaries compile with `cargo-chef` dependency caching
- [ ] `CONTRIBUTING.md` covers local development setup, test execution, and PR process
- [ ] `CHANGELOG.md` exists in Keep a Changelog format with a complete `v1.0.0` entry
- [ ] Repository is public on GitHub
- [ ] `v1.0.0` is tagged and a GitHub Release is published with release notes
- [ ] At least 5 `good first issue` issues are filed and labeled

---

## Key Dates

| Date | Event |
|------|-------|
| Monday | Sprint start — `cargo audit`; Docker image optimization; begin load test data seeding |
| Tuesday | Load test execution + performance tuning (if needed) |
| Wednesday | Security hardening; `CONTRIBUTING.md` |
| Thursday | `CHANGELOG.md` + GitHub Release draft |
| Friday EOD | Repository public. `v1.0.0` tagged. GitHub Release published. Retro. 🎉 |

---

## Beta Milestone — Summary

| Sprint | Goal | Key Deliverable |
|--------|------|-----------------|
| **6** | Multilingual + Types | Language tab UI, pre-publish completeness check, MCP language filter, OpenAPI pipeline stable |
| **7** | History + Analytics | Page version history, restore-as-draft, MCP query logging, admin analytics dashboard |
| **8** | Export + Ollama | Full-workspace markdown export, Ollama local LLM support, embedding model configuration |
| **9** | Hardening + Release | Load test, performance validation, security audit, public GitHub release `v1.0.0` |

**Total Beta timeline:** 4 weeks (Weeks 6–9).
**Full Alpha → v1.0.0 timeline:** 9 weeks from Sprint 1 start.

**v1.0 definition:** All P0 requirements complete and tested. MCP p95 < 2s at 10k chunks. Public GitHub repository with `CONTRIBUTING.md`. Installation tested by a non-author in under 30 minutes.
