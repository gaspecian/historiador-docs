# Vector Store Backfill on Boot — Design Spec

**Status:** Approved (2026-04-25)
**Branch:** `bugfix/vector_store_documentation`
**Supersedes:** none
**Related:** [ADR-007 Chronik-Stream](../adr/ADR-007-chronik-stream.md), [chronik vector store rewrite plan](2026-04-25-chronik-vector-store-rewrite.md)

## Goal

When the API boots, optionally reconcile any `published` page versions sitting in PostgreSQL that are not present in the Chronik-Stream `published-pages` topic by running them through the existing chunk pipeline. The mechanism must be safe to leave on permanently: when Postgres and Chronik agree, the boot-time cost is two SQL queries.

## Motivation

Today, page content reaches the vector store only through the publish path (`PublishPageUseCase` spawns a fire-and-forget chunk pipeline after a page transitions to `published`). If a page was published before Chronik was wired up, or if Chronik state is wiped/rebuilt independently of Postgres, those pages stay invisible to MCP queries and the existing admin reindex endpoint must be invoked per workspace.

A boot-time backfill closes this gap without requiring operator intervention per workspace.

## Scope

**In scope:**
- A boot-time background task in the `apps/api` binary that reconciles missing `published` page versions into Chronik.
- A new `GET /health/ready` endpoint that reflects backfill progress so load balancers / k8s readiness probes can hold off traffic until the vector store is consistent.
- A `BACKFILL_ON_BOOT` env var (default off) that enables the task.

**Out of scope (YAGNI for v1):**
- Backfilling `draft` page versions. Drafts stay private; matches existing publish-pipeline semantics.
- Detecting partial sync (a page version with some chunks in Chronik but not all). Same blind spot exists in the normal publish flow.
- Re-publishing already-synced page versions ("force replay"). Operators can manually `TRUNCATE` `chunks` rows and restart if they need to repopulate.
- Concurrent / parallel backfill. Serial only in v1.
- Batch size / pacing knobs. Single env var, single behavior.
- Drift detection between Postgres `chunks` row offsets and actual Chronik content.
- A REST/CLI trigger to re-run backfill without restarting the API. Restart is the trigger.

## Architecture

**Approach A — DISTINCT probe** (selected from three options during brainstorming):

1. Query Chronik via SQL: `SELECT DISTINCT page_version_id FROM "published-pages"` → `synced_set`
2. Query Postgres: all `id` from `page_versions` where the parent page is `status = 'published'` → `expected_set`
3. Compute `to_backfill = expected_set − synced_set`
4. For each id in `to_backfill`, invoke the same per-version chunk pipeline used by `PublishPageUseCase` after a publish

Trust model: a `page_version_id` appearing in Chronik *at all* is treated as "synced" — we do not verify chunk count. Re-publishing a partially-synced page would create duplicate chunks in Chronik (append-only log, no upsert). Accepting this blind spot is the v1 choice; the same blind spot exists in normal publish today.

## Boot Sequence

```
main()
  ├─ load env / build AppState (existing)
  ├─ run sqlx migrations (existing)
  ├─ probe Chronik /health (existing)
  ├─ start Axum server bound to :3001                ← API serves traffic immediately
  │     • /health           → 200 (liveness — unchanged)
  │     • /health/ready     → 200 if state ∈ {Disabled, Completed}
  │                           503 if state ∈ {Running, Failed}
  └─ if env BACKFILL_ON_BOOT is truthy:
        tokio::spawn(BackfillService::new(state.clone()).run())

BackfillService::run():
  ├─ set state = Running
  ├─ probe Chronik (3 retries: 5s, 15s, 30s backoff)
  │     on persistent failure → state = Failed { reason: "chronik probe unavailable" }, return
  ├─ load expected_set from Postgres
  ├─ to_backfill = expected_set − synced_set
  ├─ log: "backfill starting: <N> page versions to sync"
  ├─ for each id in to_backfill (serial):
  │     • run_chunk_pipeline_for(id)
  │     • on success: synced += 1
  │     • on error:   failed += 1, log WARN with page_version_id and error
  │     • every 50 pages: log INFO progress
  ├─ log INFO: "backfill complete: <synced> synced, <failed> failed"
  └─ set state = Completed { synced, skipped: 0, failed }
```

When `BACKFILL_ON_BOOT` is unset or falsy: state stays `Disabled`, no task spawns, `/health/ready` returns 200 immediately. Boot is unchanged from today.

## Configuration

| Env var | Default | Values | Effect |
|---|---|---|---|
| `BACKFILL_ON_BOOT` | `false` | `1`, `true`, `yes` (case-insensitive) → on; everything else → off | Enables boot-time backfill |

Documented in `.env.example`.

## Operational Surface

### `GET /health` (unchanged)
Liveness probe. Returns `200 {"status":"ok"}` whenever the process is alive. Not affected by backfill state.

### `GET /health/ready` (new)
Readiness probe. Mounted in the unauthenticated allowlist (same gate as `/health` and `/setup/probe` — must work during the 423 setup-locked state).

| Backfill state | Status code | Body |
|---|---|---|
| `Disabled` | 200 | `{"status":"ready","backfill":"disabled"}` |
| `Running` | 503 | `{"status":"not_ready","backfill":"running"}` |
| `Completed { synced, failed }` | 200 | `{"status":"ready","backfill":"completed","synced":<n>,"failed":<m>}` |
| `Failed { reason }` | 503 | `{"status":"not_ready","backfill":"failed","reason":"<r>"}` |

`Failed` is terminal — operator must restart the API to retry.

### Logging

All backfill log lines are emitted by the `BackfillService`. No new tracing targets needed; uses the existing `apps/api` logging setup.

- `INFO` "backfill starting: N page versions to sync"
- `INFO` "backfill progress: 50/N synced, K failed" (every 50 pages)
- `INFO` "backfill complete: N synced, K failed"
- `WARN` "backfill: page_version <uuid> failed: <error>"
- `ERROR` "backfill: chronik probe failed after retries: <error>"

No metrics endpoint, no tracing spans, no Prometheus counters in v1.

## Error Handling

**Chronik probe failure** (network down, SQL error parsing the response, etc.):
- Retry 3 times with backoff: 5s, 15s, 30s.
- If still failing, set state = `Failed`. `/health/ready` stays 503.
- Operator must restart the API to retry. Rationale: backfill cannot proceed without an authoritative `synced_set`; silently skipping would let Chronik diverge from Postgres without any signal.

**Per-page-version failure** (chunker error, Kafka produce error, Postgres error during chunk row management):
- Log `WARN` with `page_version_id` and error.
- Increment failure counter.
- Continue to the next page. **Backfill never aborts on a single-page failure.**

**Retry across runs:**
- No retry within a single backfill run. A failed page version remains missing from Chronik, so the next API boot's probe will pick it up again.

**Concurrency:**
- Serial execution. The existing `run_chunk_pipeline_for` deletes + inserts `chunks` table rows per page version; concurrent runs against overlapping versions could race. Backfill is a boot-time one-shot, not a hot path. If perf becomes a concern at scale, bounded concurrency can be added later behind another env var.

## Components & File Layout

### New files

| Path | Responsibility |
|---|---|
| `apps/api/src/infrastructure/backfill/mod.rs` | Module entry; re-exports `BackfillService`, `BackfillState` |
| `apps/api/src/infrastructure/backfill/state.rs` | `BackfillState` enum + `Arc<RwLock<BackfillState>>` shared handle |
| `apps/api/src/infrastructure/backfill/service.rs` | `BackfillService::run()` orchestrator: probe → diff → iterate |

### Modified files

| Path | Change |
|---|---|
| `apps/api/src/main.rs` | After `axum::serve(...)`, conditionally spawn `BackfillService::run()` based on env |
| `apps/api/src/state.rs` | Add `pub backfill_state: Arc<RwLock<BackfillState>>` field, init to `Disabled` |
| `apps/api/src/routes.rs` | Mount `GET /health/ready` |
| `apps/api/src/app.rs` | Add `/health/ready` to unauthenticated allowlist (alongside `/health`, `/setup/probe`) |
| `apps/api/src/presentation/handler/health.rs` (or wherever `/health` lives) | Add `health_ready_handler` that reads `AppState::backfill_state` and returns the matrix above |
| `apps/api/src/config.rs` (or equivalent) | Parse `BACKFILL_ON_BOOT` boolean |
| `crates/db/src/postgres/page_versions.rs` | Add `list_published_ids_across_workspaces(&self) -> Result<Vec<Uuid>, sqlx::Error>` |
| `crates/db/src/chronik/analytics.rs` | Add `list_synced_page_version_ids(&self) -> anyhow::Result<Vec<String>>` (wraps `query_sql` with the DISTINCT query) |
| `apps/api/src/application/pages/publish_page.rs` | Expose `run_chunk_pipeline_for` as `pub(crate)` (or extract to a free function in `infrastructure/chunker/pipeline.rs` if it's tied to use-case state today) |
| `.env.example` | Document `BACKFILL_ON_BOOT` |

### Untouched

- `crates/chunker` — no changes
- `crates/db/src/chronik/kafka_producer.rs`, `producer.rs` — no changes
- `pages` / `page_versions` / `chunks` Postgres schema — **no migration**

### OpenAPI

`/health/ready` gets a `#[utoipa::path]` annotation alongside `/health`. After the change, `pnpm gen:types` regenerates `openapi.yaml` and `packages/types/generated/`.

## Data Model

No schema changes. The reconciliation is built entirely on existing types:

- `page_versions(id UUID, page_id UUID, language TEXT, ...)` — `id` is the unit of backfill
- `pages(id UUID, status pages_status, ...)` — filter on `status = 'published'`
- `chunks(page_version_id, chronik_partition INT, chronik_offset BIGINT, ...)` — touched by the existing chunk pipeline; backfill does not read this directly (uses Chronik as the authoritative `synced_set`)
- Chronik `published-pages` topic — `page_version_id` is a top-level field in `ChunkPayload`, exposed as a column in Chronik's DataFusion view

## Testing

### Unit tests
- `BackfillService::compute_diff(expected: HashSet<Uuid>, synced: HashSet<Uuid>) -> Vec<Uuid>` — pure function. Test: empty/empty, expected=synced, expected superset of synced, synced superset of expected (orphans tolerated).
- State transitions: `Disabled → Running → Completed`; `Disabled → Running → Failed`. Verify `Completed { synced, failed }` carries the right counts.
- `health_ready_handler` returns the right code + body shape for each `BackfillState` variant. Use a mocked `AppState` with each variant.

### Integration tests
Use the same Postgres + Chronik test harness from commit `34b1d04` (end-to-end publish → MCP query against real Chronik):

1. **Cold backfill:** Seed N published page versions directly via Postgres SQL (skip the publish path so Chronik stays empty). Set `BACKFILL_ON_BOOT=true`. Boot the API. After the spawned task completes (poll `/health/ready` until 200), assert all N `page_version_id` values are returned by the Chronik DISTINCT query.
2. **Partial backfill:** Same setup, but publish M of the N pages through the normal `PublishPageUseCase` first. Assert backfill only re-publishes (N − M).
3. **Disabled mode:** `BACKFILL_ON_BOOT=false`. Boot the API. Assert `/health/ready` returns 200 immediately, no chunks appear in Chronik beyond what was seeded.
4. **Chronik down:** Stop the Chronik container. Set `BACKFILL_ON_BOOT=true`. Boot the API. Assert `/health/ready` eventually returns 503 with `backfill=failed` after the 3-retry backoff window (~50s).

## Risks & Mitigations

| Risk | Mitigation |
|---|---|
| Backfill re-publishes pages it shouldn't (DISTINCT probe gives a false positive on partial sync, so we *under*-publish — no risk of over-publish in v1) | Acceptable: same blind spot as normal publish path. Document as known limitation. |
| Chronik DataFusion view doesn't expose `page_version_id` as a column | Verified during brainstorming: `ChunkPayload.page_version_id` is a top-level JSON field; DataFusion exposes top-level keys as columns. Integration test #1 catches regressions. |
| Backfill takes too long for k8s readiness probe defaults | `/health/ready` model is "503 until done" — k8s will hold off traffic indefinitely if needed. Operators can disable backfill via env if startup window is bounded. |
| Spawned task panics silently | `tokio::spawn` returns a `JoinHandle`; a wrapper logs panic via `JoinError` and sets state = `Failed`. |
| Existing `run_chunk_pipeline_for` is private to `PublishPageUseCase` and assumes use-case-scoped state | Either expose as `pub(crate)` or extract to a free function in `infrastructure/chunker/pipeline.rs`. Decision made at plan time after reading the current signature. |

## Open Questions

None. All design decisions are settled.

## Acceptance Criteria

1. `BACKFILL_ON_BOOT=true` on a fresh deploy with N published pages in Postgres and an empty Chronik results in N pages being queryable via MCP within one boot cycle.
2. `BACKFILL_ON_BOOT=true` on a steady-state deploy (Postgres and Chronik already in sync) completes the probe phase in under 1 second and flips `/health/ready` to 200 immediately.
3. `BACKFILL_ON_BOOT=false` (default) leaves boot behavior identical to today.
4. A page-version-level error during backfill is logged with `page_version_id` and does not abort the run.
5. Chronik unavailability at boot causes `/health/ready` to report `failed` after the retry window; the API still serves `/health` (200) and other endpoints.
