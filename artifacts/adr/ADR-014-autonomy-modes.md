# ADR-014: Autonomy Is a Property of Generation Mode, Not a New Behavioral Axis

**Status:** Accepted
**Date:** 2026-04-19
**Extends:** [ADR-008](ADR-008-split-pane-editor.md), [ADR-012](ADR-012-editor-message-envelope.md), [ADR-013](ADR-013-proposal-overlay.md)
**Deciders:** Gabriel Specian (Nexian Tech)

---

## Context

Sprint 11 introduces three named autonomy levels for the AI: **Propose**, **Checkpointed**, and **Autonomous**. On first reading these look like a new behavioral axis — orthogonal to the conversation-vs-generation dichotomy already established in ADR-008. If we treated them that way, the editor would have a 2 × 3 product space (conversation × {propose, checkpointed, autonomous}, generation × {propose, checkpointed, autonomous}) with six distinct states.

Most of those combinations are nonsensical:

- "Conversation mode, autonomous" — what would the AI auto-apply? Conversation messages don't mutate the document.
- "Conversation mode, checkpointed" — same issue. There's nothing to batch and pause on.
- "Conversation mode, propose" — equivalent to plain conversation; the redundant label adds no behavior.

The three modes only meaningfully describe behavior **when the AI is actually mutating the document** — i.e., generation mode. Folding autonomy into generation mode collapses the state space from six to three.

There's also a defaults question: which mode is the workspace default? An overly aggressive default risks the trust violation ADR-013 was designed to prevent (AI rewriting content faster than the author can react). An overly cautious default makes the "AI writes the first section" flow noisier than necessary.

---

## Decision

**Autonomy is a property of generation mode only.** Conversation mode always behaves as "propose" in the trivial sense that no conversation message ever mutates the document. The three autonomy levels describe how the AI behaves when it enters generation mode:

| Mode | Behavior |
|------|----------|
| **Propose** *(default)* | Every block op the AI emits in generation mode appears as a proposal in the ADR-013 overlay. Author must approve, reject, or edit each one before it lands in the base document. |
| **Checkpointed** | The AI emits block ops into the overlay in batches, then pauses with an `autonomy_checkpoint` envelope message (ADR-012) summarizing the proposed batch. Author approves or rejects the batch, then the AI resumes. |
| **Autonomous** | Block ops apply to the base document immediately as they are emitted. They still flow through the overlay (ADR-013) so the author sees them appear, but they auto-resolve to approved after a brief visual delay. Review happens via diff in the version history, not inline. |

**Storage:** `autonomy_mode` is a column on `page_versions` (default `'propose'`), with a workspace-level default that new pages inherit. Per-page override is exposed in the editor toolbar.

**Audit:** Every change to `autonomy_mode` is logged as an event in `editor-conversations` (joining the event-type discriminator from ADR-015), so the conversation replay shows exactly when the author escalated or de-escalated the AI's authority.

**Default is `Propose`.** Confirmed by Gabriel on 2026-04-19. New users land in the safest mode; opting up is explicit.

---

## Options Considered

### Option A: Autonomy as a property of generation mode ✅ Selected

| Dimension | Assessment |
|-----------|------------|
| State space | 3 modes (propose, checkpointed, autonomous) within generation mode |
| Conceptual model | One axis the author already understands (conversation vs generation), refined |
| Code-path branching | Single dispatcher in the block-op handler |
| Default safety | `Propose` — no surprise mutations |
| Audit trail | Single column on `page_versions` + event in `editor-conversations` |

**Pros:**
- The mode is a single enum, not a matrix — UI and code stay simple
- Generation-mode dispatcher branches in one place; the rest of the system doesn't care
- Defaults are unambiguous (`Propose`); opt-up is explicit and audited
- Cleanly composes with ADR-013's overlay — autonomous mode is "auto-approve every overlay entry"
- Re-uses the proposal/ack envelope from ADR-012 for Checkpointed via `autonomy_checkpoint` / `autonomy_decision`

**Cons:**
- The label "Propose" is redundant in conversation mode (always true) — minor cognitive overhead in the toolbar copy
- Authors who want "AI auto-applies this one paragraph but proposes everything else" have no per-block override; they'd need to flip the mode mid-session

### Option B: Autonomy as an independent axis (2 × 3 matrix)

Treat autonomy as orthogonal: conversation × {propose, checkpointed, autonomous}, generation × {propose, checkpointed, autonomous}.

**Pros:**
- Maximum theoretical flexibility

**Cons:**
- Four of the six combinations have no useful behavior to define
- The toolbar UI must explain six states; the mental model is heavier than the value justifies
- Code paths multiply for combinations that will never be exercised
- Defaults become hard to explain ("conversation mode autonomy" — what does that even configure?)

**Rejected** — flexibility for combinations that are nonsensical isn't flexibility, it's surface area.

### Option C: A single global "AI authority" slider

A 0–100 slider that fades from "always asks" to "never asks". The slider position implicitly chooses the mode and the checkpoint cadence.

**Pros:**
- Smooth conceptual range; no discrete labels to defend

**Cons:**
- Authors can't reason about what a value of "47" actually does — discrete modes are easier to learn and predict
- The implementation still has to map slider position to discrete branches in code; the slider just hides the discreteness from the user
- Auditing "what mode was active at time T?" becomes "what was the slider position?", which is harder to summarize in a changelog

**Rejected** — the appearance of granularity adds confusion without behavioral benefit.

### Option D: Per-block autonomy overrides

Each block carries its own autonomy hint: this paragraph is autonomous, that section is checkpointed, etc.

**Pros:**
- Maximum per-element control

**Cons:**
- Adds a second autonomy concept that interacts with the page-level mode in confusing ways ("the page is Propose but this block is Autonomous — what wins?")
- Storage and UI surface multiply per block
- No clear use case in v1.1; speculative

**Rejected for v1.1.** Could be revisited if real per-block use cases emerge.

---

## Architecture

### Mode resolution

```
Block op arrives at generation-mode handler
  → look up page_version.autonomy_mode
    → 'propose'       → emit block_op (proposal_id), wait for block_op_ack
    → 'checkpointed'  → buffer block_op into current batch
                          → on batch boundary (heading change, N ops, T seconds):
                             emit autonomy_checkpoint with summary
                             wait for autonomy_decision
                             if approve: flush buffer through overlay → base
                             if reject: discard buffer, AI re-plans
    → 'autonomous'    → emit block_op (proposal_id) marked auto_resolve
                          → overlay shows brief diff visualization
                            → after 1.5s (configurable), auto-resolves to approved
                              → block lands in base document
```

### Checkpoint batch boundaries

Checkpointed mode batches block ops until one of:

- Heading-level change (entering or leaving a section)
- Configured op-count threshold (default 5)
- Configured time threshold (default 10 s of continuous AI emission)
- Tool-call boundary (after a `tool_result` returns)

The first heuristic that fires closes the batch and emits the checkpoint.

### Audit trail

Every mode change is recorded in `editor-conversations` as an event:

```
{
  event_type: 'autonomy_mode_changed',
  page_id,
  conversation_turn,
  from_mode: 'propose' | 'checkpointed' | 'autonomous',
  to_mode:   'propose' | 'checkpointed' | 'autonomous',
  changed_by: 'author' | 'workspace_default',
  timestamp,
}
```

Replay reconstructs the active mode at any historical moment, which is essential for "why did the AI auto-apply this change?" forensic questions.

### Toolbar UX

The toolbar exposes the current mode with explicit copy:

- **Propose** — *I'll suggest changes; you approve each one.*
- **Checkpointed** — *I'll work in batches and check in with you between them.*
- **Autonomous** — *I'll apply changes directly. You can review history.*

The label is colored/iconized to make Autonomous mode visually distinctive — an author should never be in Autonomous mode without realizing it.

---

## Consequences

**Easier:**
- One enum instead of a matrix; one branch point in the dispatcher
- Defaults are unambiguous and safe
- Re-uses ADR-013's overlay for all three modes, not three different rendering paths
- Audit trail is uniform across modes

**Harder:**
- Authors must understand three modes; toolbar copy and onboarding must explain them clearly
- Checkpointed mode's batch boundaries are heuristic — too aggressive batching feels unresponsive, too eager checkpointing feels chatty. Will need tuning based on observation
- Autonomous mode's "brief visualization delay" is a balancing act: too short and the author misses surprising changes; too long and it feels like Propose with extra steps

**Must revisit:**
- Per-user learned defaults (e.g., "Gabriel usually flips to Checkpointed for long documents") — interesting but speculative; revisit only if usage data supports it
- Per-block autonomy hints (Option D above) — same answer
- Tunable checkpoint heuristics: if the default thresholds are noticeably wrong for typical documents, expose them as workspace settings before exposing per-page

---

## Action Items

1. [ ] Add `autonomy_mode` enum column to `page_versions` (default `'propose'`)
2. [ ] Add `default_autonomy_mode` to `workspaces` table; new pages inherit
3. [ ] Implement toolbar toggle in the editor with the explicit mode-description copy
4. [ ] Implement Propose-mode dispatch (re-uses ADR-013 overlay flow)
5. [ ] Implement Checkpointed-mode batching and `autonomy_checkpoint` / `autonomy_decision` envelope handling
6. [ ] Implement Autonomous-mode auto-resolution with the configurable visualization delay (default 1.5 s)
7. [ ] Implement `autonomy_mode_changed` event logging to `editor-conversations`
8. [ ] Replay reconstruction: given a `(page_id, timestamp)`, return the active mode at that moment
9. [ ] Onboarding tooltip: when an author opts up from Propose for the first time, show a one-time confirmation describing what changes
