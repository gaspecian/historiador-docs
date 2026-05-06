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
        let row: Option<(String, String)> =
            sqlx::query_as("SELECT language, content_markdown FROM page_versions WHERE id = $1")
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
