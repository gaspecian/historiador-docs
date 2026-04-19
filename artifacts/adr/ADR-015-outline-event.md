# ADR-015: Outline Is a Durable Event Type in `editor-conversations`, Not a New Topic or Table

**Status:** Accepted
**Date:** 2026-04-19
**Extends:** [ADR-007](ADR-007-chronik-stream-adoption.md), [ADR-008](ADR-008-split-pane-editor.md)
**Deciders:** Gabriel Specian (Nexian Tech)

---

## Context

Sprint 11's conversational intake produces an **outline** before any blocks are written. The AI asks clarifying questions ("Who's the reader? What sections do you need?") and converges on a proposed structure: a list of headings with brief bullet notes about what each section will cover. The author approves (or comments on and revises) the outline; generation mode then seeds the canvas with one `heading` block per approved section.

The outline is not a block tree — the canvas doesn't have the content yet. It's an ordered list of proposed sections, which exists only during intake. Three storage choices were on the table:

1. Give the outline its own Chronik topic (`editor-outlines`).
2. Put the outline in PostgreSQL as a new table (`outlines` or `outline_versions`).
3. Store the outline as a structured event type within the existing `editor-conversations` topic.

ADR-007 is deliberately conservative about adding Chronik topics. Its Consequences section reads: *"enabling the wrong capabilities on a high-volume topic wastes resources"* and *"adding a fifth topic requires justifying the storage and operational cost against the same consumer patterns."* A new topic is not free — it adds configuration surface, retention decisions, capability flag choices, consumer path code, and monitoring.

The outline is also conversational in nature: it's the output of the intake conversation, revised through continued conversation, and approved through the commenting mechanism (ADR-016). Storing it adjacent to the rest of the conversation keeps the replay story simple.

---

## Decision

**Use `editor-conversations` with a discriminated event type.** The outline is a kind of conversation artifact and belongs where the rest of the conversation lives.

The `editor-conversations` event schema gains an `event_type` discriminator (reused by ADR-014 and ADR-016):

```json
{
  "event_type": "outline_proposed" | "outline_approved" | "outline_revised",
  "page_id": "…",
  "conversation_turn": 7,
  "outline": {
    "outline_id": "ol-abc123",
    "sections": [
      { "section_id": "s1", "heading": "Overview", "bullets": ["Why this exists", "Who it's for"] },
      { "section_id": "s2", "heading": "Getting Started", "bullets": ["Prerequisites", "First run"] }
    ]
  },
  "derived_from_message_id": "msg-…",
  "timestamp": "2026-04-19T…"
}
```

The generation-mode handler reads the **latest `outline_approved`** for the page and seeds the block tree from it — one `heading` block per section, empty `paragraph` blocks underneath as placeholders. Approval happens through the ADR-016 comment mechanism: author approves the outline wholesale or comments on specific sections to request revisions.

**No new topic. No new table.** The outline lives as a structured payload on the existing conversation event log.

---

## Options Considered

### Option A: Event type inside `editor-conversations` ✅ Selected

| Dimension | Assessment |
|-----------|------------|
| New Chronik topic | No |
| New PostgreSQL table | No |
| Replay integration | Native — replays with the rest of the conversation |
| Query patterns | Streaming-only (ADR-007) — analytics require a materialized view |
| Ordering with comments/messages | Trivial — same topic, same partition |

**Pros:**
- No new topic configuration, capability flags, retention policies, or consumer paths to maintain
- The outline is replayed with the rest of the conversation on reconnect (ADR-009) — an author reopening a tab mid-intake sees the proposed outline exactly as they left it
- Approval via comments (ADR-016) stays within the same event log — the whole intake-to-approval flow is one linear event stream
- Matches ADR-007's "conservative topic count" posture

**Cons:**
- Analytical queries over outlines (e.g., "which section headings appear most often across the workspace?") require filtering the conversation topic. With SQL-on-`editor-conversations` limited to streaming per ADR-007, analytics must go through a materialized PostgreSQL view rather than direct Chronik SQL
- The event schema gets richer; consumers must tolerate the discriminator and ignore variants they don't care about

### Option B: New Chronik topic `editor-outlines`

Separate topic for outline events with its own capability flags.

**Pros:**
- Cleaner separation; SQL analytics could be enabled independently from conversation-volume concerns
- Consumers interested only in outlines don't have to filter out conversation noise

**Cons:**
- Adds operational surface directly against ADR-007's guidance
- Cross-topic ordering between "outline_proposed" on topic A and the conversation message that produced it on topic B becomes a synchronization problem
- Duplicates replay logic for the editor — now there are two topics to replay from on reconnect
- Low traffic volume doesn't justify a dedicated topic in v1.1

**Rejected** — cost vs. benefit is upside-down at the volumes we expect. Revisit if outlines become a high-volume independently-queried entity.

### Option C: PostgreSQL table `outlines` (or `outline_versions`)

Store outlines in Postgres alongside `page_versions`.

**Pros:**
- Relational analytics are trivial (filter, aggregate, join with `pages`)
- Type-safe schema via migrations

**Cons:**
- Splits the intake conversation across two durable stores — the conversation messages are in Chronik, the outline derived from them is in Postgres. Reconstructing "what did the AI see when it proposed this outline?" requires joining across stores with different consistency guarantees
- Outline revisions are a natural event stream; a table models them awkwardly (either overwrite or append-with-version_id — neither is as natural as an event)
- Writes happen on every outline revision; cheap but adds another write path

**Rejected** — the outline is a conversational event more than a relational entity. Event-stream storage fits better.

### Option D: Derive the outline on demand from the conversation

Don't store the outline explicitly; parse it out of the conversation messages whenever needed.

**Pros:**
- Zero new schema

**Cons:**
- Parsing free-form conversation into a structured outline is error-prone
- "Approved outline" has no canonical form to point to — is it the last thing the AI said before generation started? The last thing the author said? A consensus of both?
- Generation mode needs a crisp, unambiguous input — not a parser
- Revisits of the outline (comment-driven revisions) have nothing to mutate

**Rejected** — structure erased at storage time is structure that has to be re-derived at every consumer. Bad trade.

---

## Architecture

### Event-type discriminator in `editor-conversations`

The topic gains a unified `event_type` field that identifies the payload shape. Existing conversation messages get `event_type: 'message'` (backwards-compatible — consumers that previously expected a raw message payload receive messages unchanged, just with the new discriminator added):

```
event_type: 'message' | 'outline_proposed' | 'outline_approved'
           | 'outline_revised' | 'autonomy_mode_changed'
           | 'comment_posted' | 'comment_resolved' | ...
```

ADR-014 (autonomy mode changes) and ADR-016 (comments) both register their event types here.

### Outline lifecycle

```
Conversation intake
  → AI synthesizes proposal
    → emit event { event_type: 'outline_proposed', outline, derived_from_message_id }
      → author reviews
        → approve wholesale: emit { event_type: 'outline_approved', outline_id }
        → comment on section: emit { event_type: 'comment_posted', ... } (ADR-016)
          → AI emits { event_type: 'outline_revised', outline: updated }
            → author reviews again (loop)
              → eventually: outline_approved
                → generation-mode handler reads latest outline_approved
                  → seeds block tree with heading blocks
```

### Latest-outline query

The `crates/db` layer exposes:

```rust
pub async fn get_latest_approved_outline(page_id: Uuid) -> Result<Option<Outline>>
```

Implementation: read from a PostgreSQL materialized view that projects the latest `outline_approved` event per `page_id`. The view is updated by a Chronik consumer that tails `editor-conversations`.

This keeps the hot path (outline lookup during generation-mode startup) off a Chronik streaming query and on an indexed Postgres read.

### Analytics surface

For cross-workspace outline analytics (e.g., "what are the most common first-section headings?"), a separate consumer job projects outline events into a purpose-built analytics table. Not built for v1.1; described here so future work isn't blocked by "we don't know where that data lives".

---

## Consequences

**Easier:**
- No new topic, no new capability flags, no new retention policy
- Replay is unified: reconnecting mid-intake restores the outline state alongside conversation history
- Outline approval via comments (ADR-016) flows through the same event log
- Consumers that don't care about outlines filter on `event_type` and ignore them

**Harder:**
- `editor-conversations` events now have a discriminated-union schema; consumers must be tolerant to unknown event types (log-and-drop rule, matching ADR-012's envelope discipline)
- Analytics over outlines require a materialized view or a separate consumer pipeline — more moving parts than "run SELECT on the outlines table"
- The schema for each event type must be versioned carefully; adding a new field to `outline_proposed` requires the same backwards-compatibility discipline as the `EditorMessage` union

**Must revisit:**
- If outline analytics become a high-volume query in v2 (gap-detection dashboard looking across workspaces, for example), either promote outlines to a dedicated topic with SQL analytics enabled, or materialize them into a richer analytics table
- If the intake conversation grows complex (multi-turn clarifications with multiple draft outlines in flight), consider a lightweight per-outline state machine to track "draft in progress" vs "proposed" vs "approved" explicitly rather than inferring from event order

---

## Action Items

1. [ ] Add the `event_type` discriminator to the `editor-conversations` event schema; backfill existing messages with `event_type: 'message'`
2. [ ] Document the `outline_proposed` / `outline_approved` / `outline_revised` payload shapes in the developer docs
3. [ ] Implement the intake conversation flow that emits outline events when the AI synthesizes or revises an outline
4. [ ] Implement the outline-approval flow using the ADR-016 comment mechanism (no separate approval UI)
5. [ ] Build the PostgreSQL materialized view projecting the latest `outline_approved` per page
6. [ ] Implement `get_latest_approved_outline(page_id)` in `crates/db`
7. [ ] Generation-mode handler: on entry, read the latest approved outline and seed the block tree
8. [ ] Replay test: reconnect mid-intake → outline state is reconstructed identically
9. [ ] Consumer tolerance test: a consumer running an older schema ignores unknown event types without erroring
