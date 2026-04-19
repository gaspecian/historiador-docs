//! Autonomy modes + checkpoint batcher (Sprint 11, phase A11).
//!
//! Three per-page modes (ADR-014):
//!
//! - `Propose` — every tool call lands in the overlay; the author
//!   resolves each proposal before it touches the base document.
//! - `Checkpointed` — proposals flow into the overlay in batches;
//!   every batch boundary, the agent pauses and asks the author to
//!   Continue / Revise / Skip before it drafts the next section.
//! - `Autonomous` — proposals auto-resolve after a configurable
//!   delay; the overlay still renders them so the author sees what
//!   is happening.
//!
//! This module ships the batcher that decides **when** to emit an
//! `autonomy_checkpoint` envelope. Three boundary triggers, fired
//! on whichever hits first:
//!
//! 1. a heading-level change (structural boundary),
//! 2. a 5-op threshold (keeps batches digestible),
//! 3. a 10-second inactivity timer (catches "the agent stopped").

use serde::{Deserialize, Serialize};

use super::block_ops::Proposal;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutonomyMode {
    Propose,
    Checkpointed,
    Autonomous,
}

impl AutonomyMode {
    pub fn from_db_str(s: &str) -> Self {
        match s {
            "checkpointed" => AutonomyMode::Checkpointed,
            "autonomous" => AutonomyMode::Autonomous,
            _ => AutonomyMode::Propose,
        }
    }

    pub fn as_db_str(self) -> &'static str {
        match self {
            AutonomyMode::Propose => "propose",
            AutonomyMode::Checkpointed => "checkpointed",
            AutonomyMode::Autonomous => "autonomous",
        }
    }
}

/// Why the batcher chose to emit a checkpoint. Surfaced on the
/// `autonomy_checkpoint` envelope so the UI can hint at the
/// cadence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointReason {
    HeadingBoundary,
    OpThreshold,
    Timeout,
}

/// Default per-mode tuning. Lives here (not at call sites) so tests
/// and the WS handler agree on the same thresholds.
pub const BATCH_OP_THRESHOLD: usize = 5;
pub const BATCH_TIMEOUT_SECONDS: u64 = 10;
pub const AUTONOMOUS_RESOLVE_MS: u64 = 1500;

#[derive(Debug, Default)]
pub struct CheckpointBatcher {
    buffer: Vec<Proposal>,
    /// Level of the heading the current batch belongs to, if any.
    /// A change of level triggers a heading-boundary flush.
    current_heading_level: Option<u8>,
}

impl CheckpointBatcher {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed a proposal into the batcher. Returns `Some(batch, reason)`
    /// when the batcher decides to flush, otherwise `None`. Callers
    /// that want a timeout flush should also call
    /// [`Self::flush_on_timeout`] on their 10-second tick.
    pub fn push(&mut self, proposal: Proposal) -> Option<(Vec<Proposal>, CheckpointReason)> {
        let new_level = heading_level_of(&proposal);
        let crossed_heading = match (self.current_heading_level, new_level) {
            (Some(prev), Some(curr)) => prev != curr,
            (None, Some(_)) => !self.buffer.is_empty(),
            _ => false,
        };
        if crossed_heading {
            // Flush the existing batch BEFORE accepting the new
            // heading — the heading starts the next section.
            let drained = std::mem::take(&mut self.buffer);
            self.buffer.push(proposal);
            self.current_heading_level = new_level;
            if drained.is_empty() {
                return None;
            }
            return Some((drained, CheckpointReason::HeadingBoundary));
        }
        if let Some(level) = new_level {
            self.current_heading_level = Some(level);
        }
        self.buffer.push(proposal);
        if self.buffer.len() >= BATCH_OP_THRESHOLD {
            let drained = std::mem::take(&mut self.buffer);
            self.current_heading_level = None;
            return Some((drained, CheckpointReason::OpThreshold));
        }
        None
    }

    /// Force-flush if the caller notices the batcher has been idle
    /// past the timeout. Returns an empty Option when the buffer is
    /// already empty (nothing to do).
    pub fn flush_on_timeout(&mut self) -> Option<(Vec<Proposal>, CheckpointReason)> {
        if self.buffer.is_empty() {
            return None;
        }
        let drained = std::mem::take(&mut self.buffer);
        self.current_heading_level = None;
        Some((drained, CheckpointReason::Timeout))
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }
}

fn heading_level_of(proposal: &Proposal) -> Option<u8> {
    use historiador_blocks::BlockKind;
    let block = match proposal {
        Proposal::Insert { new_block, .. } => Some(new_block),
        Proposal::Replace { replacement, .. } => Some(replacement),
        Proposal::Append { new_blocks, .. } => new_blocks.first(),
        _ => None,
    };
    match block.map(|b| &b.kind) {
        Some(BlockKind::Heading { level }) => Some(*level),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use historiador_blocks::{Block, BlockId, BlockKind};
    use uuid::Uuid;

    fn insert(kind: BlockKind) -> Proposal {
        Proposal::Insert {
            proposal_id: Uuid::now_v7(),
            anchor_block_id: BlockId::new(),
            position: crate::application::editor::block_ops::InsertPosition::After,
            new_block: Block {
                id: BlockId::new(),
                kind,
                markdown: "x\n".into(),
            },
        }
    }

    #[test]
    fn autonomy_mode_from_db_str_defaults_to_propose() {
        assert_eq!(AutonomyMode::from_db_str("propose"), AutonomyMode::Propose);
        assert_eq!(
            AutonomyMode::from_db_str("checkpointed"),
            AutonomyMode::Checkpointed
        );
        assert_eq!(
            AutonomyMode::from_db_str("autonomous"),
            AutonomyMode::Autonomous
        );
        assert_eq!(AutonomyMode::from_db_str("unknown"), AutonomyMode::Propose);
    }

    #[test]
    fn threshold_flush_fires_at_five_ops() {
        let mut b = CheckpointBatcher::new();
        for _ in 0..(BATCH_OP_THRESHOLD - 1) {
            assert!(b.push(insert(BlockKind::Paragraph)).is_none());
        }
        let flush = b.push(insert(BlockKind::Paragraph));
        assert!(matches!(flush, Some((_, CheckpointReason::OpThreshold))));
        assert!(b.is_empty());
    }

    #[test]
    fn heading_level_change_triggers_flush() {
        let mut b = CheckpointBatcher::new();
        assert!(b.push(insert(BlockKind::Heading { level: 2 })).is_none());
        assert!(b.push(insert(BlockKind::Paragraph)).is_none());
        let flush = b.push(insert(BlockKind::Heading { level: 3 }));
        assert!(matches!(
            flush,
            Some((_, CheckpointReason::HeadingBoundary))
        ));
        // The new heading stays buffered as the start of the next
        // batch.
        assert!(!b.is_empty());
    }

    #[test]
    fn timeout_flush_drains_partial_batch() {
        let mut b = CheckpointBatcher::new();
        b.push(insert(BlockKind::Paragraph));
        b.push(insert(BlockKind::Paragraph));
        let flush = b.flush_on_timeout();
        assert!(matches!(flush, Some((_, CheckpointReason::Timeout))));
        assert!(b.is_empty());
    }

    #[test]
    fn timeout_flush_on_empty_batch_is_noop() {
        let mut b = CheckpointBatcher::new();
        assert!(b.flush_on_timeout().is_none());
    }

    #[test]
    fn non_heading_first_proposal_does_not_trigger_boundary() {
        let mut b = CheckpointBatcher::new();
        assert!(b.push(insert(BlockKind::Paragraph)).is_none());
        assert!(b.push(insert(BlockKind::Heading { level: 2 })).is_some());
    }
}
