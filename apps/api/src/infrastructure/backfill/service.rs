//! Backfill orchestrator. Probes Chronik for already-synced page
//! versions, diffs against Postgres, and re-publishes the missing
//! ones through the existing chunk pipeline.

use std::collections::HashSet;

use uuid::Uuid;

pub struct BackfillService;

impl BackfillService {
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
