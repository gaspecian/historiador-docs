# ADR-010: Frontend Canvas as a Block Tree (Rendering Model, Not Storage)

**Status:** Accepted
**Date:** 2026-04-19
**Extends:** [ADR-008](ADR-008-split-pane-editor.md), [ADR-002](ADR-002-chunking-strategy.md)
**Deciders:** Gabriel Specian (Nexian Tech)

---

## Context

The right-pane canvas in ADR-008 was described abstractly as "a live markdown preview". In the current code (`apps/web`), it is literally a `const content` string holding the raw markdown. This works for whole-page regeneration but makes every Sprint 11 interaction either impossible or fragile:

- Block-level operations (`insertBlock`, `replaceBlock`, `appendToSection`, `deleteBlock`, `suggestBlockChange`) need to target a specific piece of the document — but a string has no addressable units smaller than character offsets, and offsets shift on every edit
- The proposal overlay (ADR-013) renders pending block ops as visual diffs anchored to specific blocks — anchored to what, if everything is a string?
- Inline comments (ADR-016) anchor to specific lines or blocks — line numbers shift the moment a paragraph above is inserted
- Section click-to-edit (ADR-008) needs to identify "the section the author clicked" — easy with stable IDs, awkward with substring matching

The chunker already treats the document as a tree — ADR-002 uses `comrak` to parse markdown into a CommonMark AST. The frontend should share that mental model.

This ADR is **only about the frontend rendering model**. The durable source of truth for the document remains `page_versions.content_markdown` in PostgreSQL (ADR-008's auto-save contract). A filesystem-primary storage model was discussed and rejected — see Options Considered below.

---

## Decision

**The canvas is stored on the frontend as a tree of typed blocks with stable IDs, not as a markdown string.**

```typescript
type Block =
  | { id: string; kind: 'heading'; level: 1|2|3|4|5|6; text: string }
  | { id: string; kind: 'paragraph'; text: string }
  | { id: string; kind: 'list'; ordered: boolean; items: string[] }
  | { id: string; kind: 'code'; language: string|null; text: string }
  | { id: string; kind: 'table'; rows: string[][] }
  | { id: string; kind: 'callout'; variant: 'info'|'warn'|'note'; text: string };

type Canvas = { page_id: string; blocks: Block[] };
```

- Block IDs are UUIDs minted client-side on first insertion and stable across renders
- The block tree is **derived** from the markdown stored in PostgreSQL on load
- The block tree is **serialized back** to markdown on save and stored in `page_versions.content_markdown`
- Round-trip (`markdown → blocks → markdown`) is lossless for the supported CommonMark subset; this is enforced by tests

**Storage stays in PostgreSQL.** The block tree is a rendering model that lives in the frontend's React state. It is not a durable artifact.

---

## Options Considered

### Option A: Block tree in frontend, markdown in PostgreSQL ✅ Selected

| Dimension | Assessment |
|-----------|------------|
| Block-level ops | Native — every op targets a stable ID |
| Comment / overlay anchoring | Native — anchors are block IDs |
| Storage durability | Inherited from ADR-008 (PostgreSQL) |
| Serialization complexity | Medium — round-trip must be lossless |
| Multi-instance / scaling | Trivial — Postgres is shared, frontend state is per-tab |

**Pros:**
- Stable identifiers for every actionable piece of the document
- Diff overlays (ADR-013) attach cleanly to block IDs
- Comments (ADR-016) survive insertions and deletions because they anchor to block IDs, not line numbers
- The mental model matches the chunker's AST (ADR-002), so what the author approves on the canvas is exactly what the chunker sees
- No new storage system; PostgreSQL remains the single source of truth

**Cons:**
- Round-trip serialization must be lossless for the CommonMark subset Historiador supports — anything outside that subset (raw HTML, exotic extensions) must be either normalized or rejected at save time
- The serializer must exist on both Rust (`crates/chunker`) and TypeScript (`apps/web`) sides and stay in sync; a divergence here causes silent data loss

### Option B: Markdown string + offset-based ops

Keep the current `const content` string; address block ops by character or line offsets.

**Pros:**
- Minimal change from current code

**Cons:**
- Offsets are invalidated by every edit — every block op must include "the offset was N at version V" and the server must reconcile if the document has moved on
- Comment anchors break the moment any text is inserted above them
- The proposal overlay has no stable handle to attach to
- Inherently fragile; turns every Sprint 11 feature into an offset-bookkeeping problem

**Rejected** — incompatible with the Sprint 11 protocol.

### Option C: Filesystem-primary storage (markdown files on disk, streamed to canvas)

Store each page as a markdown file on the server filesystem; AI tool calls write to the file; the file content is streamed to the canvas via WebSocket.

**Pros:**
- Files are inspectable with familiar tools (cat, grep, git)
- Could enable Git-friendly workflows for documentation-as-code

**Cons:**
- Doesn't solve the block-identity problem — the canvas still needs stable handles for ops, comments, and overlays. Re-parsing the file after every change would mean block IDs change every edit, breaking everything that anchors to them
- Local filesystem doesn't survive horizontal scaling (ADR-003 names this as the first scaling lever) — would force NFS / EFS / shared object store
- Multi-workspace isolation becomes a directory-discipline problem instead of a `workspace_id` column
- Introduces a third durable store alongside PostgreSQL and Chronik; consistency between them becomes a real concern
- AI tool calls would degenerate into "rewrite the whole file" instead of targeted block ops, killing the proposal overlay and the targeted-edit story

**Rejected for v1.1.** Filesystem export as a *mirror* of PostgreSQL (e.g., an "Export to Git" feature that reconstructs the file tree from `page_versions`) remains a reasonable v2 candidate.

---

## Architecture

### Round-trip flow

```
Page load:
  PostgreSQL.page_versions.content_markdown
    → comrak parse (server, in crates/chunker)
      → block tree
        → sent to frontend as Canvas
          → React renders blocks

Author or AI edit:
  block op applied to React state
    → re-render

Auto-save (every 30s, ADR-008):
  block tree
    → serialize to markdown (apps/web, mirroring crates/chunker logic)
      → POST to API
        → API persists to page_versions.content_markdown
          → Chunker re-parses on publish, writes chunks to Chronik
```

### Block ID lifecycle

- New blocks (inserted by author or AI) get a fresh UUID on the frontend
- On save, IDs are written into the markdown as HTML comments (`<!-- block:abc-123 -->`) preceding each block
- On reload, the parser reads the comment and reuses the ID; if absent, a fresh ID is minted
- This makes IDs stable across sessions while keeping the markdown human-readable (comments are invisible in rendered output)

### Round-trip integrity guarantee

A property test (`crates/chunker` and `apps/web` both run it) asserts: for any markdown in the supported CommonMark subset, `parse → serialize → parse` produces the same block tree (modulo whitespace normalization). CI fails if this property is violated.

---

## Consequences

**Easier:**
- Every block op targets an ID, not a substring
- Diff overlays attach to block IDs (ADR-013)
- Comments anchor to block IDs (ADR-016) and survive inserts/deletes elsewhere in the document
- Section click-to-edit (ADR-008) becomes a `block.id` reference
- The chunker (ADR-002) sees the exact same structure the author approved

**Harder:**
- Markdown ↔ block-tree serialization must be lossless for the supported CommonMark subset
- Two implementations of the serializer must stay in sync (Rust + TypeScript); changes require coordinated updates
- Pasted rich content (HTML, Word, Google Docs) may not round-trip cleanly — needs a normalization step at paste time
- Block ID comments add visual noise to the raw markdown source; acceptable trade-off but worth noting

**Must revisit:**
- If pasted HTML mangling becomes a frequent author complaint, add a "raw HTML block" escape hatch — only after measuring how often this actually happens
- Filesystem export (as a v2 mirror of PostgreSQL) is a reasonable feature; design when there's user demand
- Sub-block editing (e.g., highlighting three words within a paragraph for a comment) is currently out of scope — block granularity only in v1.1

---

## Action Items

1. [ ] Define the `Block` discriminated union in `packages/types` (generated from the OpenAPI schema so Rust and TypeScript agree)
2. [ ] Implement markdown ↔ block-tree serialization in `crates/chunker` (extend the existing `comrak` AST walker)
3. [ ] Mirror the serialization in `apps/web` for client-side rendering and round-trip on save
4. [ ] Implement block ID persistence via HTML comments in the saved markdown
5. [ ] Write round-trip property tests: for the full CommonMark subset, `markdown → blocks → markdown` is a no-op modulo whitespace normalization
6. [ ] Define normalization rules for pasted HTML / rich content at paste time
7. [ ] Document the "rendering model, not storage" framing in `CONTRIBUTING.md` so future contributors don't conflate the two
