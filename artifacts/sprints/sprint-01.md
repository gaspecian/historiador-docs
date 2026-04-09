# Sprint 1 — Foundation

**Dates:** Week 1 (5 working days)
**Team:** Gabriel Specian (solo)
**Sprint Goal:** The monorepo is scaffolded, all services start via a single `docker compose up`, the PostgreSQL schema is migrated, and the Axum API returns a working health check. Every subsequent sprint builds on top of this without rework.

---

## Why This Scope

The full PRD has six P0 requirements. Attempting all of them in week 1, solo, would produce a half-built everything and a fully-built nothing. Sprint 1 is a **foundation sprint** — its only job is to make it possible to build features cleanly in Sprint 2 onward.

A foundation sprint is complete when:
- Any developer can clone the repo and run the full stack in under 10 minutes
- The data model is defined and version-controlled via migrations
- The service boundaries (API, MCP, web) are established even if mostly empty
- CI validates every commit

Features — AI editor, chunker, MCP queries, user management — start in Sprint 2.

---

## Capacity

| Person | Available Days | Velocity | Notes |
|--------|---------------|----------|-------|
| Gabriel | 5 of 5 | 10 points | Solo — plan to 80% = 8 committed points |
| **Total** | **5** | **8 pts committed / 2 pts stretch** | |

*1 point ≈ ~half a day of focused work.*

---

## Sprint Backlog

### P0 — Must Ship (8 points)

| # | Item | Points | Notes |
|---|------|--------|-------|
| 1 | **Monorepo scaffold** | 2 pts | Initialize Cargo workspace (`crates/api`, `crates/mcp`, `crates/chunker`, `crates/db`, `crates/llm`) + pnpm workspaces (`apps/web`, `packages/types`) + Turborepo config. Basic `turbo build` must pass end-to-end. |
| 2 | **Docker Compose stack** | 2 pts | All services defined: `web` (Next.js), `api` (Axum), `mcp` (Axum), `postgres`, `vexfs`. `docker compose up` starts everything. Services that aren't built yet return a placeholder. Persistent volumes configured. |
| 3 | **PostgreSQL schema + migrations** | 2 pts | Define and migrate all v1 tables: `workspaces`, `users`, `sessions`, `collections` (nested, adjacency list), `pages`, `page_versions` (per-language content), `chunks` (metadata only — embeddings live in VexFS). Use `sqlx-cli` for migrations. |
| 4 | **Axum API skeleton** | 1 pt | `GET /health` returns `{ status: "ok", version }`. Router structure in place for future route groups (`/auth`, `/pages`, `/collections`, `/admin`). `sqlx` pool connected to PostgreSQL. Structured logging with `tracing`. |
| 5 | **CI pipeline** | 1 pt | GitHub Actions: on every PR — `cargo build`, `cargo test`, `cargo clippy --deny warnings`, `pnpm install`, `pnpm build`. Must pass before merge. |

### P1 — Ship If Time Allows (2 points stretch)

| # | Item | Points | Notes |
|---|------|--------|-------|
| 6 | **Next.js app scaffold** | 1 pt | Basic Next.js app in `apps/web`. Placeholder home page. Confirms frontend builds and serves correctly within Docker Compose. |
| 7 | **OpenAPI generation pipeline** | 1 pt | Wire `utoipa` into the Axum API to emit `openapi.yaml` at build time. Run `openapi-typescript` in the Turborepo pipeline to generate `packages/types/generated/`. Confirms the Rust→TypeScript type contract works end-to-end. |

### Explicitly Out of Scope (Sprint 2+)

- Authentication / JWT / sessions
- Any user-facing feature (AI editor, page creation, collections UI)
- VexFS client integration (blocked until VexFS Rust client is confirmed — see Risk 1)
- Chunker implementation
- MCP protocol implementation
- LLM integration

---

## Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| **VexFS has no Rust client** | VexFS container can't be wired to the API — `crates/db` has a stub instead of a real client | Before Day 1: confirm with your father's team whether a Rust client exists. If not, scaffold a trait interface and use a HTTP stub for now. Plan the client implementation as a Sprint 2 item. |
| **comrak / sqlx unfamiliar** | Unexpected ramp-up time on Rust library APIs slows the schema and chunker work | Budget the first half of Day 1 to read `sqlx` migration docs and `comrak` AST traversal examples before writing production code. |
| **Docker build times for Rust** | Cold Rust builds in Docker can take 10–20 minutes without layer caching | Set up `cargo-chef` in the Dockerfile from the start (Day 2). Do not optimize later — it's much harder to retrofit. |
| **Solo — no code review** | Architectural mistakes go unreviewed and compound | Write at least one reviewer note per PR as if explaining to a future contributor. Self-review using `cargo clippy` as a proxy for a human reviewer. |

---

## Day-by-Day Plan

| Day | Focus | Goal |
|-----|-------|------|
| **Monday** | Monorepo scaffold | `cargo build` and `pnpm build` both pass from the repo root via Turborepo. Repo pushed to GitHub. |
| **Tuesday** | Docker Compose + Postgres | `docker compose up` starts all containers. PostgreSQL is reachable. `cargo-chef` Dockerfile in place. |
| **Wednesday** | Schema + migrations | All tables created via `sqlx-cli migrate run`. Schema is documented and version-controlled in `crates/db/migrations/`. |
| **Thursday** | Axum API skeleton + CI | `GET /health` returns 200. GitHub Actions CI passes on a test PR. |
| **Friday** | Stretch + buffer | OpenAPI pipeline if time allows. Otherwise: write the Sprint 2 backlog and do a personal retro. |

---

## Definition of Done

- [ ] `git clone` + `docker compose up` brings the full stack to a running state in under 10 minutes on a fresh machine
- [ ] All PostgreSQL tables exist and are created by reproducible `sqlx` migrations
- [ ] `cargo clippy --deny warnings` passes with zero warnings
- [ ] CI passes on every commit
- [ ] `GET /api/health` returns `200 OK`
- [ ] Sprint 2 backlog is drafted before Friday EOD

---

## Key Dates

| Date | Event |
|------|-------|
| Monday | Sprint start — monorepo scaffold |
| Wednesday | Mid-sprint check: Docker Compose and schema on track? |
| Friday EOD | Sprint end — demo: `docker compose up` and `curl /api/health` |
| Friday EOD | Sprint 2 backlog drafted |

---

## Sprint 2 Preview (don't build this week)

Sprint 2 will be the first **feature sprint**. Likely scope:
- Authentication (JWT, workspace setup endpoint)
- First-run setup wizard API (language config, admin account)
- Page + collection CRUD endpoints
- VexFS client in `crates/db` (if Rust client confirmed)
- Next.js dashboard scaffold with auth flow
