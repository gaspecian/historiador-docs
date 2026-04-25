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
        *s.write().unwrap() = BackfillState::Completed {
            synced: 5,
            failed: 1,
        };
        let guard = s.read().unwrap();
        match &*guard {
            BackfillState::Completed { synced, failed } => {
                assert_eq!(*synced, 5);
                assert_eq!(*failed, 1);
            }
            _ => panic!("expected Completed"),
        }
    }
}
