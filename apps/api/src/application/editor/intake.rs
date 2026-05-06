//! Conversational intake gating (Sprint 11, phase A8 / US-11.01).
//!
//! The agent's first turn on a fresh page is discovery: it asks 2–4
//! clarifying questions instead of dumping markdown. This module
//! computes which [`PromptMode`] the renderer should use for the
//! next turn given three inputs:
//!
//! - whether the canvas has any structural content,
//! - whether an outline has already been approved, and
//! - whether the user hit "Skip discovery" in the composer.
//!
//! Keeping the decision here (pure function, no I/O) means the WS
//! handler and the test harness agree on the boundary.

use super::prompt_template::PromptMode;

#[derive(Debug, Default, Clone, Copy)]
pub struct IntakeState {
    /// True when the current canvas has at least one non-empty block.
    pub canvas_has_content: bool,
    /// True when the user approved an outline at any point during
    /// this session. Persists across turns — does not reset if the
    /// user later edits the outline.
    pub outline_approved: bool,
    /// True when the user clicked Skip discovery this session.
    /// Equivalent to "don't ask me, just write".
    pub skip_discovery: bool,
}

/// Given the intake state and whether the upcoming turn is meant to
/// mutate the canvas, pick a `PromptMode`.
///
/// Rules:
/// 1. Blank canvas AND no outline AND not-skipped ⇒ `Intake` (ask questions).
/// 2. User explicitly asked for a change ⇒ `Generation`.
/// 3. Otherwise ⇒ `Conversation`.
pub fn determine_mode(state: IntakeState, user_requested_generation: bool) -> PromptMode {
    if user_requested_generation {
        return PromptMode::Generation;
    }
    let needs_discovery =
        !state.canvas_has_content && !state.outline_approved && !state.skip_discovery;
    if needs_discovery {
        PromptMode::Intake
    } else {
        PromptMode::Conversation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_canvas_without_outline_triggers_intake() {
        let mode = determine_mode(IntakeState::default(), false);
        assert!(matches!(mode, PromptMode::Intake));
    }

    #[test]
    fn approved_outline_suppresses_intake() {
        let state = IntakeState {
            outline_approved: true,
            ..Default::default()
        };
        let mode = determine_mode(state, false);
        assert!(matches!(mode, PromptMode::Conversation));
    }

    #[test]
    fn canvas_content_suppresses_intake() {
        let state = IntakeState {
            canvas_has_content: true,
            ..Default::default()
        };
        let mode = determine_mode(state, false);
        assert!(matches!(mode, PromptMode::Conversation));
    }

    #[test]
    fn skip_discovery_suppresses_intake() {
        let state = IntakeState {
            skip_discovery: true,
            ..Default::default()
        };
        let mode = determine_mode(state, false);
        assert!(matches!(mode, PromptMode::Conversation));
    }

    #[test]
    fn user_requested_generation_overrides_everything() {
        let mode = determine_mode(IntakeState::default(), true);
        assert!(matches!(mode, PromptMode::Generation));

        let state = IntakeState {
            canvas_has_content: true,
            outline_approved: true,
            skip_discovery: true,
        };
        let mode = determine_mode(state, true);
        assert!(matches!(mode, PromptMode::Generation));
    }
}
