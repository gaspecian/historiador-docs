# ADR-016: Inline Comments Are a First-Class Conversation Input Channel

**Status:** Accepted
**Date:** 2026-04-19
**Extends:** [ADR-008](ADR-008-split-pane-editor.md), [ADR-010](ADR-010-canvas-block-tree.md), [ADR-012](ADR-012-editor-message-envelope.md), [ADR-015](ADR-015-outline-event.md)
**Deciders:** Gabriel Specian (Nexian Tech)

---

## Context

Sprint 11's editor gives the author two ways to talk to the AI:

1. **Left-pane conversation** — global, topical ("rewrite the introduction in a friendlier tone")
2. **Section click-to-edit** (ADR-008 + ADR-010) — refocuses AI attention on one block

Neither covers the feedback pattern authors reach for most naturally when reviewing a draft: *"On these two specific lines, do X"* or *"On the whole document, do Y"*. The author wants to highlight one or more blocks — or scope the comment to the whole page — attach a comment, and send that comment to the AI to act on. The AI either acknowledges the comment in conversation or proposes block ops anchored to the commented blocks.

This is familiar from Google Docs and Cursor's inline chat. It carries architectural weight because it's a new persistence path, a new wire-protocol variant, and a new LLM input shape.

It also resolves an open question from earlier discussion: **how does the author approve the intake outline (ADR-015)?** With a commenting mechanism, no separate approval UI is needed. The author either approves the outline wholesale (single click on an "Approve outline" affordance) or comments on specific sections to request revisions. Same mechanism as all other feedback.

---

## Decision

**Inline comments are durable events on `editor-conversations` that flow into the LLM as structured inputs.** Specifically:

- A comment is anchored to either a set of block IDs (one or more, contiguous or not) **or** to the whole page
- Comments are posted by the author, consumed by the LLM, and optionally resolved by the author when addressed
- Comments **never** mutate the document directly — they are inputs to the AI, which responds via the existing block-op (ADR-013) or conversation flow
- Comments persist in `editor-conversations` as discrete event types (`comment_posted`, `comment_resolved`), joining the event-type discriminator established in ADR-015

**Wire format (extends ADR-012):**

```typescript
type EditorMessage =
  // ... existing variants (ADR-008, ADR-012) ...
  | { type: 'comment_posted'; seq: number; comment_id: string;
      anchor: { kind: 'blocks'; block_ids: string[] } | { kind: 'whole_page' };
      text: string }
  | { type: 'comment_resolved'; seq: number; comment_id: string };
```

**LLM input shape:** when the AI receives a `comment_posted` event, the conversation handler formats it as a structured user message that includes:

1. The comment text
2. The anchor context — the actual rendered text of the anchored blocks
3. The `comment_id` so the AI's response can reference it

The AI can respond in conversation mode (acknowledge, ask a clarifying question) or generation mode (emit `block_op` messages targeting the anchored block IDs). Any resulting block ops carry a `proposal_id` tagged with the source `comment_id` for audit traceability.

**Resolution model:** comments have one of two states — open or resolved. A resolved comment is not deleted; it remains in `editor-conversations` history with a `comment_resolved` event. The LLM's system prompt surfaces only **open** comments as active work items each turn. The UI can hide resolved comments by default but keep them accessible.

**Outline approval reuses this mechanism.** No separate approval UI. The author either clicks "Approve outline" (which emits `outline_approved` per ADR-015) or comments on specific sections (which the AI handles by emitting `outline_revised`).

---

## Options Considered

### Option A: Comments as first-class events on `editor-conversations` ✅ Selected

| Dimension | Assessment |
|-----------|------------|
| New persistence store | None — reuses `editor-conversations` |
| Wire protocol | Two new variants on the existing envelope (ADR-012) |
| LLM input | Structured user message with anchor context |
| Audit trail | Native — comments and resolutions in event log |
| Outline approval reuse | Yes — same mechanism |

**Pros:**
- Outline approval UX (ADR-015) is solved by the same mechanism — no separate interface to design
- Structured review flows ("rewrite this paragraph in a less technical tone") fall out naturally instead of needing dedicated primitives
- Audit trail is preserved — every comment and resolution is in `editor-conversations` alongside the rest of the conversation history
- The feature mirrors patterns authors already know (Google Docs comments, Cursor inline chat), reducing cognitive load
- One conversation log to reason about; no parallel comments-store to keep consistent
- Reconnect/replay (ADR-009) restores comments alongside conversation state for free

**Cons:**
- The UI must render anchored highlights on the canvas plus a sidebar listing open comments — non-trivial visual-design work; must coexist with ADR-013's proposal overlay without clashing visually
- The LLM's system prompt must explicitly enumerate open comments as active work items each turn — adds tokens to every generation request proportional to the comment backlog
- Orphaned anchors (a commented block is later deleted or replaced) need explicit handling

### Option B: Comments in their own PostgreSQL table

Store comments in a relational table, link by `(page_id, block_id)` foreign keys.

**Pros:**
- Clean relational schema; easy to query and aggregate
- Standard CRUD endpoints

**Cons:**
- Splits author-AI feedback across two stores: comments in Postgres, conversation messages in Chronik. Reconstructing "what did the AI see when it proposed this revision?" requires cross-store joins
- Comment lifecycle (post → AI responds → maybe revise → resolve) is naturally an event stream; a table models it awkwardly
- Adds a new write path with its own consistency story
- Doesn't leverage the existing replay-on-reconnect machinery — comment state on reconnect would need its own restore logic

**Rejected** — comments are conversational events; storing them as a relational entity loses the natural fit.

### Option C: Comments as plain conversation messages with anchor metadata

No new event type — just add an optional `anchor` field to the existing `conversation` message type.

**Pros:**
- Smallest schema change

**Cons:**
- Conflates two distinct things: free-form conversation in the left pane vs. anchored review feedback on the right pane
- Resolution state has no natural home (a `conversation` message isn't "resolved")
- The UI distinction between "this is a chat message" and "this is a review comment" gets muddled because the underlying type is the same
- Outline approval flow doesn't fit cleanly — approval isn't a conversation message

**Rejected** — the modeling is genuinely different; collapsing them creates ambiguity in both directions.

### Option D: Comments as ephemeral UI state, not persisted

Comments live only in the editor session and are sent to the AI as part of the next conversation message.

**Pros:**
- Zero persistence work

**Cons:**
- Closing the tab loses all unaddressed comments
- No audit trail
- Multi-tab on the same page (per ADR-009's per-tab socket model) gets divergent comment state
- Resolution tracking is impossible without persistence

**Rejected** — comments are exactly the kind of work-in-progress feedback that needs to survive a closed tab.

---

## Architecture

### Anchor model

```typescript
type CommentAnchor =
  | { kind: 'blocks'; block_ids: string[] }  // one or more block IDs
  | { kind: 'whole_page' }
```

- `blocks` anchors highlight the listed blocks visually on the canvas
- `whole_page` anchors appear in the comment sidebar without per-block highlighting; rendered as "On the whole document"
- Block IDs are stable across edits (ADR-010), so anchors survive insertions and deletions of *other* blocks elsewhere in the document

### Lifecycle

```
Author selects 1+ blocks (or "comment on whole page")
  → emit comment_posted { comment_id, anchor, text }
    → server persists to editor-conversations (event_type: 'comment_posted')
      → AI conversation handler picks up the event
        → formats anchor context for LLM (block text rendered as-is)
          → next AI turn sees the comment in the system prompt
            → AI responds:
              - conversation message (acknowledge, clarify), OR
              - block_op proposals tagged with proposal_id linked to comment_id
                → ADR-013 overlay handles approve/reject as usual
                  → author manually marks comment resolved when satisfied
                    → emit comment_resolved { comment_id }
                      → server persists to editor-conversations
                        → AI no longer surfaces comment in subsequent system prompts
```

### Orphaned-anchor handling

When a comment anchors to blocks that are later deleted (by author edit or by an approved AI block op), the anchor is orphaned. Handling rule:

- If **all** anchored block IDs are gone, the comment is treated as `whole_page` scope and a UI badge marks it as orphaned
- If **some** anchored block IDs remain, the anchor is narrowed to the surviving subset and a UI badge marks it as partial
- Orphaned/partial state is not a separate event type; it's derived from comparing anchor block IDs against the current canvas

The author can re-anchor an orphaned comment by selecting new blocks and clicking "re-anchor", which emits a `comment_posted` for a new comment_id and a `comment_resolved` for the original.

### LLM system prompt format

Each generation-mode prompt includes an "Open comments" section:

```
Open comments requiring action:
1. [comment_id: c-abc] On blocks [b-001, b-002]: "Make this less jargony."
   Anchor text:
   ---
   <rendered text of b-001>
   <rendered text of b-002>
   ---

2. [comment_id: c-def] On the whole page: "Add a 'getting started' section near the top."
```

Conversation-mode prompts include the same section so the AI can ask clarifying questions about open comments before acting on them.

### Visual design coexistence

ADR-013's proposal overlay highlights blocks in green/red (insert/delete) or with a diff view. ADR-016's comment anchors highlight blocks with a margin marker and a side accent. The two highlight schemes are designed to be visually distinct (different colors, different placement) so a block that simultaneously has a pending proposal and an open comment is unambiguously both.

### Outline approval flow

```
AI emits outline_proposed (ADR-015)
  → outline rendered in canvas as a list of proposed sections
    → author either:
      (a) clicks "Approve outline" → emit outline_approved
      (b) selects one or more sections, posts comment → emit comment_posted
        → AI revises → emit outline_revised
          → loop until approve
```

No separate approval UI exists. The "Approve outline" affordance and the comment mechanism cover all paths.

---

## Consequences

**Easier:**
- Outline approval UX is solved by the same mechanism; no separate approval interface to design
- Structured review flows fall out naturally
- Audit trail is preserved across the full conversation
- Mirrors patterns authors already know
- Replay/reconnect restores comments without extra logic

**Harder:**
- The UI must render anchored highlights and a comment sidebar; the visual design must coexist with ADR-013's proposal overlay
- The LLM system prompt grows with the open-comment count; large backlogs cost tokens. Mitigation: cap the number of open comments surfaced per turn (default 10 most recent) with a "and N more" suffix
- Orphaned-anchor handling is real and needs explicit UX
- When an AI block op proposal addresses a comment, the proposal acceptance shouldn't auto-resolve the comment — the author may want to verify the change addressed their concern. Comments are author-resolved, not AI-resolved

**Must revisit:**
- Threaded replies on a comment (author ↔ AI back-and-forth inside a single comment) — would be valuable but adds complexity. Out of scope for v1.1; revisit based on usage signals
- @-mentioning teammates — future multi-author feature; not applicable until real-time collaboration is in scope
- Sub-block anchors (highlighting three words within a paragraph) — v1.1 anchors at block granularity only; sub-block ranges are a future refinement
- AI-suggested comment resolution ("I addressed this — should I mark resolved?") — possible UX improvement once we have data on how often authors forget to resolve

---

## Action Items

1. [ ] Add `comment_posted` and `comment_resolved` variants to the `EditorMessage` union in `packages/types`
2. [ ] Add `event_type: 'comment_posted' | 'comment_resolved'` to the `editor-conversations` schema (joining the discriminator from ADR-015)
3. [ ] Implement canvas-side UI: select blocks → "Add comment" action → comment sidebar with open/resolved filter
4. [ ] Implement comment anchor rendering on the canvas, visually distinct from ADR-013's proposal overlay
5. [ ] Implement LLM input formatting for comments (anchor text included verbatim, `comment_id` passed through)
6. [ ] Tag block-op `proposal_id`s with the originating `comment_id` when the AI responds to a comment
7. [ ] Implement open-comments surfacing in the generation-mode and conversation-mode system prompts (cap at 10 most recent with "and N more")
8. [ ] Implement orphaned-anchor detection and the partial/orphaned UI badges
9. [ ] Implement re-anchor flow (resolve original comment + post new comment with new anchor)
10. [ ] Use the comment mechanism to back the outline-approval flow; remove any separate outline approval UI from the design
11. [ ] Integration test: post comment → AI responds with block op → approve op → manually resolve comment → comment no longer in system prompt
12. [ ] Integration test: post comment → delete anchored block → comment shows as orphaned with `whole_page` fallback scope
