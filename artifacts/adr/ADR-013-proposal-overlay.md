# ADR-013: Proposed Block Ops Are a Diff Overlay, Not a Mutation of the Saved Document

**Status:** Accepted
**Date:** 2026-04-19
**Extends:** [ADR-008](ADR-008-split-pane-editor.md), [ADR-010](ADR-010-canvas-block-tree.md), [ADR-012](ADR-012-editor-message-envelope.md)
**Deciders:** Gabriel Specian (Nexian Tech)

---

## Context

When the AI proposes a block op (Sprint 11), the author needs to see the proposed change clearly — without it being saved to the actual document. Two specific failure modes drive this requirement:

1. **The 30-second auto-save (ADR-008) fires while a proposal is still pending.** If proposed AI content is treated as a mutation of the canvas, the auto-save silently writes unapproved AI output into the author's document. That's a trust violation.
2. **The author closes the tab before approving a proposal.** Same outcome — AI output landed in the persisted document because the author never explicitly accepted it.

At the same time, **generation-mode writes** (ADR-008's "write the first section" flow) *do* save automatically, because the author requested them. The model must distinguish "AI is proposing" from "AI is writing what I asked for" without making the author manage two different concepts.

---

## Decision

**The right pane renders two layers:**

1. **Base document** — the persisted `page_versions.content_markdown` (parsed into a block tree per ADR-010). Auto-saved every 30 seconds and on every `generation_complete` event. **Never contains unapproved AI output.**
2. **Proposal overlay** — an in-memory layer of pending `BlockOp`s keyed by `proposal_id` (ADR-012). Rendered visually distinct: green-highlighted for inserts, red strikethrough for deletes, side-by-side or inline diff for replaces.

The author resolves each proposal with **approve / reject / edit**. Only approved ops are applied to the base document. Auto-save flushes only the base document.

**Generation-mode writes flow through the same overlay path** but with `autonomy_mode = autonomous` (ADR-014) — they auto-resolve to approved as they arrive, landing in the base document immediately. This keeps **one code path for both flows**: every AI mutation goes through the overlay, and the autonomy mode determines whether resolution is automatic or manual.

The overlay state is persisted to the `editor-conversations` Chronik topic (so it survives reconnect per ADR-009) but is **never** persisted to `page_versions`.

---

## Options Considered

### Option A: Two-layer rendering (base + overlay) ✅ Selected

| Dimension | Assessment |
|-----------|------------|
| Trust guarantee | Mechanically enforced — unapproved AI never auto-saved |
| Resolution UX | Native — approve/reject/edit per proposal |
| Generation-mode integration | Same code path with autonomy mode flag |
| Rendering complexity | Medium — requires conflict-aware overlay renderer |
| Reconnect behavior | Overlay replays from `editor-conversations` |

**Pros:**
- The rule "unapproved AI content is never auto-saved" is guaranteed by the layering, not by coding discipline
- Generation, proposal, and comment-driven flows all share one mutation path
- The proposal overlay is naturally durable across reconnect (it's in the event log)
- Author's view of the document and the persisted document never silently diverge

**Cons:**
- Rendering a block tree with a pending overlay of block ops requires careful state management
- Multiple overlapping proposals on the same block can occur (rare but real — e.g., AI proposes `replaceBlock` on block X while the author is editing block X) and need explicit conflict UX

### Option B: Inline auto-apply with undo

The AI's proposed change is applied directly to the base document; an undo stack lets the author revert.

**Pros:**
- Simpler rendering — no overlay layer

**Cons:**
- Trust violation is permanent if the author closes the tab before noticing the change
- Undo stacks don't survive reconnect cleanly
- Does not distinguish "AI proposed" from "I wrote this" in the document history
- Auto-save can race with undo, producing weird states ("I just undid that — why is it saved?")

**Rejected** — the trust failure mode is the entire problem this ADR exists to solve.

### Option C: Separate review screen

Proposals queue up in a side panel; author reviews them on a separate screen and applies in batches.

**Pros:**
- No overlay complexity in the canvas

**Cons:**
- Breaks the conversational flow — author has to context-switch to a separate screen
- Inconsistent with ADR-008's split-pane single-surface model
- Defeats the "see the document forming as you talk" feedback loop

**Rejected** — incompatible with the editor's design philosophy.

---

## Architecture

### Layer composition

```
Render frame:
  base_blocks: Block[]                      // from page_versions.content_markdown
  pending_proposals: Map<proposal_id, BlockOp>  // overlay
    →
  rendered_blocks = compose(base_blocks, pending_proposals)
    where compose() applies each proposal as a visual diff
    on top of the base, marking blocks with their proposal status
```

### Conflict detection

When a proposal targets a block that the author has edited locally since the proposal was emitted, the overlay marks the proposal as **stale** and surfaces:

> *"The AI's suggestion is now stale because you've edited this block. Apply anyway, edit further, or discard?"*

Stale proposals are not silently overwritten. The author chooses what to do.

### Resolution paths

```
approve(proposal_id):
  pending_proposals.delete(proposal_id)
    → base_blocks = apply_op(base_blocks, op)
      → emit block_op_ack { outcome: 'applied' }
        → next auto-save flushes new base to PostgreSQL

reject(proposal_id):
  pending_proposals.delete(proposal_id)
    → emit block_op_ack { outcome: 'rejected' }
      → AI handler may re-plan based on rejection signal

edit(proposal_id, edited_op):
  pending_proposals.delete(proposal_id)
    → base_blocks = apply_op(base_blocks, edited_op)
      → emit block_op_ack { outcome: 'edited', final_op: edited_op }
        → next auto-save flushes
```

### Autonomous mode shortcut

When `autonomy_mode = autonomous`, the overlay still receives the `block_op` (so it appears as a brief diff visualization for the author's awareness) but auto-resolves to `approved` after a short visual delay. The author can intervene if they spot something problematic, but inaction is acceptance.

---

## Consequences

**Easier:**
- The rule "unapproved AI content is never auto-saved" is mechanically guaranteed by the layering — not by coding discipline
- The author's view always reflects what would happen if they walked away (base = persisted)
- Conflict detection is explicit instead of being a silent overwrite
- One code path for generation, proposals, and comment-driven edits

**Harder:**
- Two-layer rendering with conflict awareness requires careful React state management
- Multiple overlapping proposals on the same block need explicit UX treatment
- The overlay must persist to `editor-conversations` for replay but be suppressed from `page_versions` — a rule that's easy to violate accidentally if not checked in tests
- Visual design: green/red highlighting and diff rendering must coexist with the comment-anchor highlighting from ADR-016 without clashing

**Must revisit:**
- If usability testing surfaces confusion about the overlay model, evaluate showing proposals **inline with an Apply button** rather than as a diff view. The data model supports both; only the rendering changes
- If autonomous mode's "brief visualization" delay confuses authors (either too long or too short), make it configurable
- Conflict resolution is currently per-proposal; if multi-author collaboration ever lands, this becomes a CRDT problem

---

## Action Items

1. [ ] Implement the two-layer rendering in the Next.js editor — base tree + overlay tree composition
2. [ ] Implement conflict detection: a proposal targeting a block edited locally since the proposal was emitted is marked stale
3. [ ] Implement approve / reject / edit handlers, each emitting `block_op_ack`
4. [ ] Persist the overlay state to `editor-conversations` for reconnect replay
5. [ ] Suppress overlay state from `page_versions` writes (test that auto-save never includes pending proposals)
6. [ ] Add a visible "N pending proposals" indicator in the editor toolbar
7. [ ] Implement the autonomous-mode brief-visualization delay (default 1.5 s) with a "pause" affordance for the author to intervene
8. [ ] Integration test: AI proposes block op → author closes tab → reopens → proposal is still pending in overlay
9. [ ] Integration test: AI proposes block op → 30 s auto-save fires → persisted document does not contain proposal
