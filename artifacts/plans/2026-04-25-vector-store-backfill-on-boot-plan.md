# Vector Store Backfill on Boot — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Spec:** [2026-04-25-vector-store-backfill-on-boot-spec.md](./2026-04-25-vector-store-backfill-on-boot-spec.md)

**Goal:** Reconcile `published` page_versions in PostgreSQL into Chronik's `published-pages` topic at API boot when `BACKFILL_ON_BOOT=true`, surfaced via a new `/health/ready` endpoint.

**Architecture:** Background `tokio::spawn` after `axum::serve(...)` runs `BackfillService::run()`: probe Chronik via SQL `SELECT DISTINCT page_version_id`, set-diff against published `page_versions` in Postgres, iterate the missing IDs through the existing `ChunkPipeline` trait (`DefaultChunkPipeline` from `apps/api/src/infrastructure/chunker/pipeline.rs`). State shared via `Arc<RwLock<BackfillState>>` on `AppState`; `/health/ready` reads it.

**Tech Stack:** Rust workspace; `axum` 0.x, `sqlx` for Postgres, `rskafka` (via existing `ChronikClient`) for Kafka writes, Chronik DataFusion REST `/_sql` for probe, `utoipa` for OpenAPI annotation.

**Branch:** `bugfix/vector_store_documentation`

---

## Task Order Overview

1. Add `BackfillState` enum + state holder
2. Add Postgres `list_published_page_version_ids` query
3. Add Chronik `list_synced_page_version_ids` SQL wrapper
4. Add `BackfillService::compute_diff` (pure function) + tests
5. Wire `BackfillState` into `AppState`
6. Add `BackfillService::run` orchestrator (probe with retries, iterate, update state)
7. Add `/health/ready` handler with `#[utoipa::path]`
8. Mount `/health/ready` route + add to setup_gate allowlist
9. Add `BACKFILL_ON_BOOT` env parsing in `main.rs` and conditionally spawn
10. Update `.env.example` and regenerate OpenAPI types
11. Integration test: cold backfill end-to-end

---

### Task 1: BackfillState enum + state holder

**Files:**
- Create: `apps/api/src/infrastructure/backfill/mod.rs`
- Create: `apps/api/src/infrastructure/backfill/state.rs`
- Modify: `apps/api/src/infrastructure/mod.rs` (add `pub mod backfill;`)

- [ ] **Step 1: Locate the infrastructure module file**

Run: `cat apps/api/src/infrastructure/mod.rs`
Expected: shows existing `pub mod` declarations like `chunker`, `crypto`, etc.

- [ ] **Step 2: Write the failing test**

Create `apps/api/src/infrastructure/backfill/state.rs` with the test at the bottom:

```rust
//! Shared state for the boot-time vector store backfill task.

use std::sync::{Arc, RwLock};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackfillState {
    /// `BACKFILL_ON_BOOT` is unset / falsy — the task never spawned.
    Disabled,
    /// Spawned task is currently working through the diff.
    Running,
    /// Task finished. May have per-page failures (`failed > 0`) but
    /// the run itself completed.
    Completed { synced: usize, failed: usize },
    /// Probe phase failed after all retries; the task gave up.
    /// Operator must restart the API to retry.
    Failed { reason: String },
}

pub type SharedBackfillState = Arc<RwLock<BackfillState>>;

pub fn shared_disabled() -> SharedBackfillState {
    Arc::new(RwLock::new(BackfillState::Disabled))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_disabled_starts_disabled() {
        let s = shared_disabled();
        assert_eq!(*s.read().unwrap(), BackfillState::Disabled);
    }

    #[test]
    fn state_can_transition_through_lifecycle() {
        let s = shared_disabled();
        *s.write().unwrap() = BackfillState::Running;
        assert_eq!(*s.read().unwrap(), BackfillState::Running);
        *s.write().unwrap() = BackfillState::Completed { synced: 5, failed: 1 };
        match &*s.read().unwrap() {
            BackfillState::Completed { synced, failed } => {
                assert_eq!(*synced, 5);
                assert_eq!(*failed, 1);
            }
            _ => panic!("expected Completed"),
        }
    }
}
```

- [ ] **Step 3: Create the module entry**

Create `apps/api/src/infrastructure/backfill/mod.rs`:

```rust
//! Boot-time vector store backfill — reconciles published page versions
//! in PostgreSQL into Chronik's `published-pages` topic.

pub mod state;

pub use state::{shared_disabled, BackfillState, SharedBackfillState};
```

- [ ] **Step 4: Add the module to infrastructure**

Open `apps/api/src/infrastructure/mod.rs`. Add `pub mod backfill;` alongside the existing modules (alphabetical position).

- [ ] **Step 5: Run the tests**

Run: `cargo test -p historiador_api infrastructure::backfill::state`
Expected: 2 passed; 0 failed.

- [ ] **Step 6: Commit**

```bash
git add apps/api/src/infrastructure/mod.rs apps/api/src/infrastructure/backfill/
git commit -m "feat(backfill): add BackfillState enum and shared holder"
```

---

### Task 2: Postgres list_published_page_version_ids

**Files:**
- Modify: `crates/db/src/postgres/page_versions.rs` (add new function at end)

- [ ] **Step 1: Write the failing test**

Append to `crates/db/src/postgres/page_versions.rs` (inside or after the existing module — there's no `#[cfg(test)] mod tests` block today; add one):

```rust
/// Cross-workspace list of every page_version `id` whose parent page is
/// `status = 'published'`. Used by the boot-time backfill to compute
/// the diff against Chronik. Workspace-agnostic — backfill is a global
/// reconciliation, not a per-tenant action.
pub async fn list_published_page_version_ids(pool: &PgPool) -> anyhow::Result<Vec<Uuid>> {
    let rows: Vec<(Uuid,)> = sqlx::query_as(
        "SELECT pv.id FROM page_versions pv \
           JOIN pages p ON p.id = pv.page_id \
          WHERE p.status = 'published'",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}
```

There's no test harness on this file today (existing functions don't have unit tests — they rely on integration tests). Skip writing a unit test for this query; its behavior will be exercised by Task 11's integration test.

- [ ] **Step 2: Build to verify no compile error**

Run: `cargo build -p historiador_db`
Expected: compiles clean.

- [ ] **Step 3: Commit**

```bash
git add crates/db/src/postgres/page_versions.rs
git commit -m "feat(db): list_published_page_version_ids cross-workspace query"
```

---

### Task 3: Chronik list_synced_page_version_ids

**Files:**
- Modify: `crates/db/src/chronik/analytics.rs` (add function alongside existing `query_sql` and `mcp_query_stats`)

- [ ] **Step 1: Add the function**

Append before the closing `}` of `impl ChronikClient` in `crates/db/src/chronik/analytics.rs`:

```rust
    /// Returns every `page_version_id` currently present in the
    /// `published-pages` topic. Used by the boot-time backfill to
    /// compute the set of versions that still need to be pushed.
    ///
    /// Returns an empty list if the topic exists but is empty. Errors
    /// (Chronik down, SQL parse failure) bubble up so the caller can
    /// retry / fail loudly — backfill cannot proceed without an
    /// authoritative synced-set.
    pub async fn list_synced_page_version_ids(&self) -> anyhow::Result<Vec<String>> {
        let resp = self
            .query_sql("SELECT DISTINCT page_version_id FROM \"published-pages\"")
            .await?;

        Ok(resp
            .rows
            .into_iter()
            .filter_map(|row| {
                row.get("page_version_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .collect())
    }
```

- [ ] **Step 2: Build to verify no compile error**

Run: `cargo build -p historiador_db`
Expected: compiles clean.

- [ ] **Step 3: Commit**

```bash
git add crates/db/src/chronik/analytics.rs
git commit -m "feat(db): chronik list_synced_page_version_ids SQL probe"
```

---

### Task 4: BackfillService::compute_diff (pure function) + tests

**Files:**
- Create: `apps/api/src/infrastructure/backfill/service.rs`
- Modify: `apps/api/src/infrastructure/backfill/mod.rs` (re-export `BackfillService`)

- [ ] **Step 1: Write the failing test in a new file**

Create `apps/api/src/infrastructure/backfill/service.rs` with just the pure function and tests:

```rust
//! Backfill orchestrator. Probes Chronik for already-synced page
//! versions, diffs against Postgres, and re-publishes the missing
//! ones through the existing chunk pipeline.

use std::collections::HashSet;

use uuid::Uuid;

pub struct BackfillService;

impl BackfillService {
    /// Returns the page_version_ids that exist in Postgres (`expected`)
    /// but not in Chronik (`synced`). Order is deterministic by sort —
    /// makes log output and tests stable.
    pub fn compute_diff(expected: &[Uuid], synced: &[Uuid]) -> Vec<Uuid> {
        let synced_set: HashSet<Uuid> = synced.iter().copied().collect();
        let mut diff: Vec<Uuid> = expected
            .iter()
            .copied()
            .filter(|id| !synced_set.contains(id))
            .collect();
        diff.sort();
        diff
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uuid(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    #[test]
    fn diff_empty_when_all_synced() {
        let expected = vec![uuid(1), uuid(2)];
        let synced = vec![uuid(1), uuid(2)];
        assert!(BackfillService::compute_diff(&expected, &synced).is_empty());
    }

    #[test]
    fn diff_returns_missing_only() {
        let expected = vec![uuid(1), uuid(2), uuid(3)];
        let synced = vec![uuid(2)];
        assert_eq!(
            BackfillService::compute_diff(&expected, &synced),
            vec![uuid(1), uuid(3)]
        );
    }

    #[test]
    fn diff_ignores_chronik_orphans() {
        // Chronik has IDs Postgres no longer knows about. Backfill
        // must NOT try to "un-sync" them; it only fills gaps.
        let expected = vec![uuid(1)];
        let synced = vec![uuid(1), uuid(99)];
        assert!(BackfillService::compute_diff(&expected, &synced).is_empty());
    }

    #[test]
    fn diff_empty_inputs() {
        assert!(BackfillService::compute_diff(&[], &[]).is_empty());
    }

    #[test]
    fn diff_full_when_chronik_empty() {
        let expected = vec![uuid(1), uuid(2)];
        assert_eq!(
            BackfillService::compute_diff(&expected, &[]),
            vec![uuid(1), uuid(2)]
        );
    }
}
```

- [ ] **Step 2: Re-export from the module**

Update `apps/api/src/infrastructure/backfill/mod.rs`:

```rust
//! Boot-time vector store backfill — reconciles published page versions
//! in PostgreSQL into Chronik's `published-pages` topic.

pub mod service;
pub mod state;

pub use service::BackfillService;
pub use state::{shared_disabled, BackfillState, SharedBackfillState};
```

- [ ] **Step 3: Run the tests**

Run: `cargo test -p historiador_api infrastructure::backfill`
Expected: 7 passed (2 from state.rs + 5 from service.rs).

- [ ] **Step 4: Commit**

```bash
git add apps/api/src/infrastructure/backfill/
git commit -m "feat(backfill): pure compute_diff with tests"
```

---

### Task 5: Wire BackfillState into AppState

**Files:**
- Modify: `apps/api/src/state.rs` (add field)
- Modify: `apps/api/src/main.rs` (initialize field)

- [ ] **Step 1: Add field to AppState**

In `apps/api/src/state.rs`, add to the imports:

```rust
use crate::infrastructure::backfill::SharedBackfillState;
```

And add this field at the end of the struct (after `editor_metrics`):

```rust
    /// Boot-time vector store backfill task state. `Disabled` when
    /// `BACKFILL_ON_BOOT` is off; otherwise transitions through
    /// `Running` → `Completed | Failed`. Read by `/health/ready`.
    pub backfill_state: SharedBackfillState,
```

- [ ] **Step 2: Initialize in main.rs**

In `apps/api/src/main.rs`, add to imports near the existing `historiador_api::` block:

```rust
use historiador_api::infrastructure::backfill::shared_disabled;
```

In the `Arc::new(AppState { ... })` literal at line 226, add:

```rust
        backfill_state: shared_disabled(),
```

After the existing `editor_metrics` field.

- [ ] **Step 3: Build to verify**

Run: `cargo build -p historiador_api`
Expected: compiles clean.

- [ ] **Step 4: Commit**

```bash
git add apps/api/src/state.rs apps/api/src/main.rs
git commit -m "feat(backfill): wire SharedBackfillState into AppState"
```

---

### Task 6: BackfillService::run orchestrator

**Files:**
- Modify: `apps/api/src/infrastructure/backfill/service.rs` (add `run` method)

- [ ] **Step 1: Replace the file with the orchestrator**

Overwrite `apps/api/src/infrastructure/backfill/service.rs`. Keep the existing `compute_diff` function and `tests` module exactly; add imports, struct, constants, `new`, `run`, `probe_chronik_with_retries`, and `run_one` around them. Final non-test contents:

```rust
//! Backfill orchestrator. Probes Chronik for already-synced page
//! versions, diffs against Postgres, and re-publishes the missing
//! ones through the existing chunk pipeline.

use std::collections::HashSet;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use sqlx::PgPool;
use tokio::time::sleep;
use uuid::Uuid;

use historiador_db::chronik::ChronikClient;
use historiador_db::postgres::page_versions;
use historiador_db::vector_store::VectorStore;

use crate::domain::port::chunk_pipeline::{ChunkPipeline, ChunkPipelineInput};
use crate::domain::value::Language;
use crate::infrastructure::backfill::state::{BackfillState, SharedBackfillState};
use crate::infrastructure::chunker::pipeline::DefaultChunkPipeline;

pub struct BackfillService {
    pool: PgPool,
    chronik: ChronikClient,
    vector_store: Arc<dyn VectorStore>,
    state: SharedBackfillState,
}

/// Backoff schedule for the Chronik probe. Three retries total:
/// fail → 5s → fail → 15s → fail → 30s → fail → give up.
const PROBE_BACKOFF: &[Duration] = &[
    Duration::from_secs(5),
    Duration::from_secs(15),
    Duration::from_secs(30),
];

impl BackfillService {
    pub fn new(
        pool: PgPool,
        chronik: ChronikClient,
        vector_store: Arc<dyn VectorStore>,
        state: SharedBackfillState,
    ) -> Self {
        Self {
            pool,
            chronik,
            vector_store,
            state,
        }
    }

    /// Returns the page_version_ids that exist in Postgres (`expected`)
    /// but not in Chronik (`synced`). Sorted for deterministic test
    /// and log output.
    pub fn compute_diff(expected: &[Uuid], synced: &[Uuid]) -> Vec<Uuid> {
        let synced_set: HashSet<Uuid> = synced.iter().copied().collect();
        let mut diff: Vec<Uuid> = expected
            .iter()
            .copied()
            .filter(|id| !synced_set.contains(id))
            .collect();
        diff.sort();
        diff
    }

    /// Run the backfill: probe Chronik, diff against Postgres, iterate
    /// missing page versions through the chunk pipeline, update state.
    /// Never panics — all errors are logged and reflected in state.
    pub async fn run(self) {
        *self.state.write().expect("backfill state lock poisoned") = BackfillState::Running;

        let synced = match self.probe_chronik_with_retries().await {
            Ok(s) => s,
            Err(reason) => {
                tracing::error!(%reason, "backfill: chronik probe failed after retries");
                *self.state.write().expect("backfill state lock poisoned") =
                    BackfillState::Failed { reason };
                return;
            }
        };

        let expected = match page_versions::list_published_page_version_ids(&self.pool).await {
            Ok(v) => v,
            Err(e) => {
                let reason = format!("postgres list query failed: {e}");
                tracing::error!(%reason, "backfill: postgres query failed");
                *self.state.write().expect("backfill state lock poisoned") =
                    BackfillState::Failed { reason };
                return;
            }
        };

        let to_backfill = Self::compute_diff(&expected, &synced);
        let total = to_backfill.len();
        tracing::info!(
            total,
            expected = expected.len(),
            already_synced = synced.len(),
            "backfill: starting"
        );

        let pipeline = DefaultChunkPipeline::new(self.pool.clone(), self.vector_store.clone());
        let mut synced_ok = 0usize;
        let mut failed = 0usize;

        for (i, page_version_id) in to_backfill.iter().enumerate() {
            match self.run_one(&pipeline, *page_version_id).await {
                Ok(()) => synced_ok += 1,
                Err(e) => {
                    failed += 1;
                    tracing::warn!(
                        %page_version_id,
                        error = %e,
                        "backfill: page version failed"
                    );
                }
            }
            if (i + 1) % 50 == 0 {
                tracing::info!(
                    progress = i + 1,
                    total,
                    synced = synced_ok,
                    failed,
                    "backfill: progress"
                );
            }
        }

        tracing::info!(synced = synced_ok, failed, "backfill: complete");
        *self.state.write().expect("backfill state lock poisoned") = BackfillState::Completed {
            synced: synced_ok,
            failed,
        };
    }

    async fn probe_chronik_with_retries(&self) -> Result<Vec<Uuid>, String> {
        let mut attempt = 0usize;
        loop {
            match self.chronik.list_synced_page_version_ids().await {
                Ok(strings) => {
                    let parsed: Vec<Uuid> = strings
                        .into_iter()
                        .filter_map(|s| Uuid::from_str(&s).ok())
                        .collect();
                    return Ok(parsed);
                }
                Err(e) => {
                    if attempt >= PROBE_BACKOFF.len() {
                        return Err(format!("chronik probe unavailable: {e}"));
                    }
                    let wait = PROBE_BACKOFF[attempt];
                    tracing::warn!(
                        attempt = attempt + 1,
                        wait_secs = wait.as_secs(),
                        error = %e,
                        "backfill: chronik probe failed, retrying"
                    );
                    sleep(wait).await;
                    attempt += 1;
                }
            }
        }
    }

    async fn run_one(
        &self,
        pipeline: &DefaultChunkPipeline,
        page_version_id: Uuid,
    ) -> anyhow::Result<()> {
        // Narrow row read: only the columns the chunk pipeline needs.
        // Avoids loading the full `PageVersion` struct.
        let row: Option<(String, String)> = sqlx::query_as(
            "SELECT language, content_markdown FROM page_versions WHERE id = $1",
        )
        .bind(page_version_id)
        .fetch_optional(&self.pool)
        .await?;

        let Some((language_str, markdown)) = row else {
            anyhow::bail!("page_version {page_version_id} no longer exists");
        };

        // `from_trusted` skips re-validation — the language was
        // validated when the page_version was first created.
        let language = Language::from_trusted(language_str);

        pipeline
            .run(ChunkPipelineInput {
                page_version_id,
                language,
                markdown,
            })
            .await
            .map_err(|e| anyhow::anyhow!("chunk pipeline failed: {e}"))?;
        Ok(())
    }
}
```

> **Note for executing agent:** the existing `compute_diff` test module from Task 4 is unchanged — keep the `#[cfg(test)] mod tests { ... }` block at the bottom of the file as-is. Removing the standalone `pub struct BackfillService;` from Task 4 (the unit-only stub) and replacing with the full struct is intentional: tests still compile because they call `BackfillService::compute_diff(...)` as an associated function.

- [ ] **Step 2: Build**

Run: `cargo build -p historiador_api`
Expected: compiles clean.

- [ ] **Step 3: Run unit tests still pass**

Run: `cargo test -p historiador_api infrastructure::backfill`
Expected: 7 passed (5 from `compute_diff` tests + 2 from `state` tests).

- [ ] **Step 4: Commit**

```bash
git add apps/api/src/infrastructure/backfill/service.rs
git commit -m "feat(backfill): orchestrator with probe retries and per-page logging"
```

---

### Task 7: /health/ready handler

**Files:**
- Create: `apps/api/src/presentation/handler/health_ready.rs`
- Modify: `apps/api/src/presentation/handler/mod.rs` (add `pub mod health_ready;`)
- Modify: `apps/api/src/presentation/openapi.rs` (register the new path + schema)

- [ ] **Step 1: Write the failing test**

Create `apps/api/src/presentation/handler/health_ready.rs`:

```rust
//! Readiness probe — reports the boot-time vector store backfill
//! status. Distinct from `/health` (liveness) so load balancers can
//! hold off traffic until backfill completes without ever reporting
//! the process as dead.

use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::{extract::State, Json};
use serde::Serialize;
use std::sync::Arc;

use crate::infrastructure::backfill::BackfillState;
use crate::state::AppState;

#[derive(Serialize, utoipa::ToSchema)]
pub struct ReadyResponse {
    pub status: &'static str,
    pub backfill: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub synced: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[utoipa::path(
    get,
    path = "/health/ready",
    responses(
        (status = 200, description = "service is ready (or backfill disabled / completed)", body = ReadyResponse),
        (status = 503, description = "backfill is running or failed", body = ReadyResponse),
    ),
    tag = "system"
)]
pub async fn handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let snapshot = state
        .backfill_state
        .read()
        .expect("backfill state lock poisoned")
        .clone();
    response_for(&snapshot)
}

fn response_for(state: &BackfillState) -> (StatusCode, Json<ReadyResponse>) {
    match state {
        BackfillState::Disabled => (
            StatusCode::OK,
            Json(ReadyResponse {
                status: "ready",
                backfill: "disabled",
                synced: None,
                failed: None,
                reason: None,
            }),
        ),
        BackfillState::Running => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ReadyResponse {
                status: "not_ready",
                backfill: "running",
                synced: None,
                failed: None,
                reason: None,
            }),
        ),
        BackfillState::Completed { synced, failed } => (
            StatusCode::OK,
            Json(ReadyResponse {
                status: "ready",
                backfill: "completed",
                synced: Some(*synced),
                failed: Some(*failed),
                reason: None,
            }),
        ),
        BackfillState::Failed { reason } => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ReadyResponse {
                status: "not_ready",
                backfill: "failed",
                synced: None,
                failed: None,
                reason: Some(reason.clone()),
            }),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_returns_200_ready() {
        let (code, body) = response_for(&BackfillState::Disabled);
        assert_eq!(code, StatusCode::OK);
        assert_eq!(body.status, "ready");
        assert_eq!(body.backfill, "disabled");
    }

    #[test]
    fn running_returns_503_not_ready() {
        let (code, body) = response_for(&BackfillState::Running);
        assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body.status, "not_ready");
        assert_eq!(body.backfill, "running");
    }

    #[test]
    fn completed_returns_200_with_counts() {
        let (code, body) = response_for(&BackfillState::Completed {
            synced: 3,
            failed: 1,
        });
        assert_eq!(code, StatusCode::OK);
        assert_eq!(body.backfill, "completed");
        assert_eq!(body.synced, Some(3));
        assert_eq!(body.failed, Some(1));
    }

    #[test]
    fn failed_returns_503_with_reason() {
        let (code, body) = response_for(&BackfillState::Failed {
            reason: "kaboom".into(),
        });
        assert_eq!(code, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body.backfill, "failed");
        assert_eq!(body.reason.as_deref(), Some("kaboom"));
    }
}
```

- [ ] **Step 2: Register the module**

Open `apps/api/src/presentation/handler/mod.rs`. Add `pub mod health_ready;` alphabetically near `pub mod health;`.

- [ ] **Step 3: Run handler tests**

Run: `cargo test -p historiador_api presentation::handler::health_ready`
Expected: 4 passed.

- [ ] **Step 4: Register in OpenAPI**

Open `apps/api/src/presentation/openapi.rs`. Two changes:
- Add `crate::presentation::handler::health_ready::handler` to the `paths(...)` list (look for where `health::handler` is referenced and add the readiness handler beside it).
- Add `crate::presentation::handler::health_ready::ReadyResponse` to the `schemas(...)` list (alongside `HealthResponse`).

Build to confirm:

Run: `cargo build -p historiador_api`
Expected: compiles clean.

- [ ] **Step 5: Commit**

```bash
git add apps/api/src/presentation/handler/health_ready.rs apps/api/src/presentation/handler/mod.rs apps/api/src/presentation/openapi.rs
git commit -m "feat(api): /health/ready handler reflecting backfill state"
```

---

### Task 8: Mount /health/ready route + setup_gate allowlist

**Files:**
- Modify: `apps/api/src/app.rs` (mount the route)
- Modify: `apps/api/src/presentation/middleware/setup_gate.rs` (add to allowlist)

- [ ] **Step 1: Mount the route**

In `apps/api/src/app.rs`, locate the existing line:
```rust
        .route("/health", get(health::handler))
```
Add the readiness route on the next line:
```rust
        .route("/health/ready", get(health_ready::handler))
```

Add the import alongside the existing `health` import. Update:
```rust
use crate::presentation::handler::health;
```
to:
```rust
use crate::presentation::handler::{health, health_ready};
```

- [ ] **Step 2: Allow through setup gate**

In `apps/api/src/presentation/middleware/setup_gate.rs`, update `is_allowed_pre_setup`:

```rust
fn is_allowed_pre_setup(path: &str) -> bool {
    matches!(
        path,
        "/health"
            | "/health/ready"
            | "/setup/status"
            | "/setup/init"
            | "/setup/probe"
            | "/setup/ollama-models"
    )
}
```

- [ ] **Step 3: Build**

Run: `cargo build -p historiador_api`
Expected: compiles clean.

- [ ] **Step 4: Commit**

```bash
git add apps/api/src/app.rs apps/api/src/presentation/middleware/setup_gate.rs
git commit -m "feat(api): mount /health/ready and allow it pre-setup"
```

---

### Task 9: BACKFILL_ON_BOOT env parsing + spawn from main.rs

**Files:**
- Modify: `apps/api/src/main.rs`

- [ ] **Step 1: Parse the env var**

In `apps/api/src/main.rs`, near where `editor_v2_enabled` is parsed (around line 62), add:

```rust
    // Boot-time vector store backfill. Default OFF so dev/test boots
    // stay fast; enable in production to reconcile any published
    // page_versions sitting in Postgres but not yet in Chronik.
    let backfill_on_boot = std::env::var("BACKFILL_ON_BOOT")
        .map(|v| matches!(v.to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false);
```

- [ ] **Step 2: Conditionally spawn after serve binds**

In `apps/api/src/main.rs`, locate the `axum::serve(...)` block (around line 250). Today it ends with `.await?;` followed by `Ok(())`. Restructure to spawn the backfill task **before** awaiting the server (since `axum::serve(...).await` blocks until shutdown):

Replace:
```rust
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}
```

With:
```rust
    // Spawn the boot-time backfill before handing control to the server.
    // The server still binds and serves traffic immediately; only
    // /health/ready reflects the in-progress backfill (returns 503
    // while Running).
    if backfill_on_boot {
        if let Some(chronik_client) = state.chronik.clone() {
            let pool = state.pool.clone();
            let vector_store = state.vector_store.clone();
            let backfill_state = state.backfill_state.clone();
            tokio::spawn(async move {
                let svc = historiador_api::infrastructure::backfill::BackfillService::new(
                    pool,
                    chronik_client,
                    vector_store,
                    backfill_state,
                );
                svc.run().await;
            });
            tracing::info!("backfill: spawned (BACKFILL_ON_BOOT=true)");
        } else {
            tracing::warn!(
                "BACKFILL_ON_BOOT=true but no Chronik client configured — \
                 skipping backfill (in-memory vector store cannot be probed)"
            );
        }
    } else {
        tracing::info!("backfill: disabled (BACKFILL_ON_BOOT not set / falsy)");
    }

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}
```

- [ ] **Step 3: Build**

Run: `cargo build -p historiador_api`
Expected: compiles clean.

- [ ] **Step 4: Lint**

Run: `cargo clippy -p historiador_api --all-targets -- -D warnings`
Expected: no warnings.

- [ ] **Step 5: Commit**

```bash
git add apps/api/src/main.rs
git commit -m "feat(api): spawn backfill on boot when BACKFILL_ON_BOOT=true"
```

---

### Task 10: Update .env.example + regenerate OpenAPI types

**Files:**
- Modify: `.env.example`
- Modify: `openapi.yaml` (regenerated)
- Modify: `packages/types/generated/index.ts` (regenerated)

- [ ] **Step 1: Document the env var**

Open `.env.example`. Add (group with other API behavior flags such as `EDITOR_V2_ENABLED`):

```
# Boot-time vector store backfill — when true, the API spawns a
# background task on startup that pushes any published page_versions
# missing from Chronik's published-pages topic. Idempotent: cheap
# when Postgres and Chronik already agree. Status surfaced via
# GET /health/ready (returns 503 while running).
BACKFILL_ON_BOOT=false
```

- [ ] **Step 2: Regenerate the OpenAPI contract and TS types**

Run: `pnpm gen:types`
Expected: `gen:openapi` writes the updated `openapi.yaml`; `build:types` writes `packages/types/generated/index.ts`. Both files now reference `/health/ready` and the `ReadyResponse` schema.

- [ ] **Step 3: Verify the contract**

Run: `grep -A 3 '/health/ready' openapi.yaml | head -20`
Expected: shows the new path with a 200 and 503 response.

Run: `grep 'ReadyResponse' packages/types/generated/index.ts | head -5`
Expected: shows the generated TypeScript type.

- [ ] **Step 4: Commit**

```bash
git add .env.example openapi.yaml packages/types/generated/index.ts
git commit -m "docs+gen: document BACKFILL_ON_BOOT, regen OpenAPI for /health/ready"
```

---

### Task 11: Integration test — cold backfill end-to-end

**Files:**
- Modify or extend the integration test that already exercises `publish → MCP query against real Chronik` (commit `34b1d04`). Find it first.

- [ ] **Step 1: Locate the existing E2E test**

Run: `grep -rln 'publish.*MCP\|published-pages.*test\|chronik.*integration' apps/api/tests/ apps/api/src/ 2>/dev/null | head -10`
Read the matching files; pick the harness that already brings up Postgres + Chronik (likely `apps/api/tests/mcp_e2e.rs` or similar).

- [ ] **Step 2: Add a new test in the same harness file**

Append a new `#[tokio::test]` (matching the harness's setup pattern). The test:

1. Boots Postgres + Chronik via the existing harness (whatever helper sets up `pool` and `ChronikClient`).
2. Seeds the workspace + a page + a published page_version directly via SQL (no `PublishPageUseCase` — that would publish through Chronik and defeat the test). Use a small markdown body like `# Hello\n\nWorld.`.
3. Verifies via `chronik.list_synced_page_version_ids()` that the page_version_id is **not** in Chronik yet.
4. Constructs a `BackfillService` and calls `.run().await` directly (no need to spawn — sync test execution).
5. Asserts the shared state is `Completed { synced: 1, failed: 0 }`.
6. Asserts a fresh `chronik.list_synced_page_version_ids()` now contains the page_version_id.

The test name: `backfill_publishes_missing_page_versions_to_chronik`. Use the existing harness's seed helpers; do not duplicate setup.

- [ ] **Step 3: Run**

Run: `cargo test -p historiador_api --test '*' backfill_publishes_missing -- --ignored` (the existing chronik harness tests are typically `#[ignore]`-gated; check the existing test attribute and match it).
If the harness is not ignore-gated: `cargo test -p historiador_api --test '*' backfill_publishes_missing`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add apps/api/tests/
git commit -m "test(backfill): cold backfill end-to-end against real Chronik"
```

---

## Final verification (after all tasks)

- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] `cargo build --workspace`
- [ ] Manual smoke: with `BACKFILL_ON_BOOT=true`, `cargo run -p historiador_api --bin api` boots, logs `backfill: spawned`, then logs `backfill: complete: 0 synced, 0 failed` (assuming clean state); `curl localhost:3001/health/ready` returns `{"status":"ready","backfill":"completed","synced":0,"failed":0}`.
- [ ] With `BACKFILL_ON_BOOT=false` (default), boot logs `backfill: disabled`; `curl localhost:3001/health/ready` returns `{"status":"ready","backfill":"disabled"}`.

---

## Acceptance criteria mapping (from spec)

| Spec criterion | Plan task |
|---|---|
| 1. Fresh deploy with N pages, empty Chronik → all N queryable in MCP after boot | Task 11 (integration) |
| 2. Steady state → probe completes in <1s, ready flips to 200 immediately | Tasks 3, 6, 7 (DISTINCT probe + state machine) |
| 3. `BACKFILL_ON_BOOT=false` → boot unchanged | Task 9 (env gate); Task 5 (default Disabled) |
| 4. Per-page failure logs and continues | Task 6 (`run_one` returns Result, loop catches) |
| 5. Chronik unavailable → ready=failed after retry window | Task 6 (`probe_chronik_with_retries`) |
