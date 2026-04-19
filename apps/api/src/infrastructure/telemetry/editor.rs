//! Editor-v2 counters. Shared across the WebSocket handler, block-op
//! dispatcher, autonomy batcher, and proposal overlay acks.

use std::sync::atomic::{AtomicU64, Ordering};

/// Autonomy mode label for `autonomy_mode_selected` increments. Kept
/// as an enum so the call sites cannot drift from the database CHECK
/// constraint that lands in Phase A11.
#[derive(Debug, Clone, Copy)]
pub enum AutonomyMode {
    Propose,
    Checkpointed,
    Autonomous,
}

impl AutonomyMode {
    fn as_str(self) -> &'static str {
        match self {
            AutonomyMode::Propose => "propose",
            AutonomyMode::Checkpointed => "checkpointed",
            AutonomyMode::Autonomous => "autonomous",
        }
    }
}

/// Atomic counters for the Sprint 11 success metrics. Cloning is cheap
/// (the struct holds `Arc`-able atomics via composition in AppState).
#[derive(Debug, Default)]
pub struct EditorMetrics {
    discovery_question_count: AtomicU64,
    diff_accept: AtomicU64,
    diff_reject: AtomicU64,
    canvas_overwrite_events: AtomicU64,
    autonomy_propose: AtomicU64,
    autonomy_checkpointed: AtomicU64,
    autonomy_autonomous: AtomicU64,
    checkpoint_hit: AtomicU64,
}

impl EditorMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_discovery_questions(&self, n: u64) {
        self.discovery_question_count
            .fetch_add(n, Ordering::Relaxed);
    }

    pub fn record_diff_accept(&self) {
        self.diff_accept.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_diff_reject(&self) {
        self.diff_reject.fetch_add(1, Ordering::Relaxed);
    }

    /// Logs and counts a canvas-overwrite attempt. Target value is 0 —
    /// any non-zero count means a tool call slipped the ADR-010 block
    /// anchor check. The runtime guard in the dispatcher (A4) refuses
    /// the op before it applies, but we still count the attempt so the
    /// dashboard can alert.
    pub fn record_canvas_overwrite_attempt(&self) {
        self.canvas_overwrite_events.fetch_add(1, Ordering::Relaxed);
        tracing::error!(
            "canvas_overwrite_events incremented — an AI tool call attempted a full-canvas replace. \
             ADR-010 invariant breached; investigate tool schema or provider adapter."
        );
    }

    pub fn record_autonomy_selected(&self, mode: AutonomyMode) {
        let counter = match mode {
            AutonomyMode::Propose => &self.autonomy_propose,
            AutonomyMode::Checkpointed => &self.autonomy_checkpointed,
            AutonomyMode::Autonomous => &self.autonomy_autonomous,
        };
        counter.fetch_add(1, Ordering::Relaxed);
        tracing::debug!(mode = mode.as_str(), "autonomy_mode_selected");
    }

    pub fn record_checkpoint_hit(&self) {
        self.checkpoint_hit.fetch_add(1, Ordering::Relaxed);
    }

    /// Snapshot for the diagnostics endpoint. Values are eventually
    /// consistent under concurrent writes, which is fine for dashboards.
    pub fn snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            discovery_question_count: self.discovery_question_count.load(Ordering::Relaxed),
            diff_accept: self.diff_accept.load(Ordering::Relaxed),
            diff_reject: self.diff_reject.load(Ordering::Relaxed),
            canvas_overwrite_events: self.canvas_overwrite_events.load(Ordering::Relaxed),
            autonomy_propose: self.autonomy_propose.load(Ordering::Relaxed),
            autonomy_checkpointed: self.autonomy_checkpointed.load(Ordering::Relaxed),
            autonomy_autonomous: self.autonomy_autonomous.load(Ordering::Relaxed),
            checkpoint_hit: self.checkpoint_hit.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct MetricsSnapshot {
    pub discovery_question_count: u64,
    pub diff_accept: u64,
    pub diff_reject: u64,
    pub canvas_overwrite_events: u64,
    pub autonomy_propose: u64,
    pub autonomy_checkpointed: u64,
    pub autonomy_autonomous: u64,
    pub checkpoint_hit: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_metrics_are_zero() {
        let m = EditorMetrics::new();
        let s = m.snapshot();
        assert_eq!(s.discovery_question_count, 0);
        assert_eq!(s.diff_accept, 0);
        assert_eq!(s.diff_reject, 0);
        assert_eq!(s.canvas_overwrite_events, 0);
        assert_eq!(s.checkpoint_hit, 0);
    }

    #[test]
    fn counters_increment() {
        let m = EditorMetrics::new();
        m.record_discovery_questions(3);
        m.record_diff_accept();
        m.record_diff_accept();
        m.record_diff_reject();
        m.record_autonomy_selected(AutonomyMode::Checkpointed);
        m.record_autonomy_selected(AutonomyMode::Checkpointed);
        m.record_autonomy_selected(AutonomyMode::Propose);
        m.record_checkpoint_hit();

        let s = m.snapshot();
        assert_eq!(s.discovery_question_count, 3);
        assert_eq!(s.diff_accept, 2);
        assert_eq!(s.diff_reject, 1);
        assert_eq!(s.autonomy_checkpointed, 2);
        assert_eq!(s.autonomy_propose, 1);
        assert_eq!(s.autonomy_autonomous, 0);
        assert_eq!(s.checkpoint_hit, 1);
    }

    #[test]
    fn overwrite_counter_fires() {
        let m = EditorMetrics::new();
        m.record_canvas_overwrite_attempt();
        assert_eq!(m.snapshot().canvas_overwrite_events, 1);
    }
}
