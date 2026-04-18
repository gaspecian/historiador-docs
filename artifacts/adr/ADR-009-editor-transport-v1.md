# ADR-009: Editor Transport for v1.0 — SSE Ratified, WebSocket Deferred

**Status:** Accepted
**Date:** 2026-04-18
**Partially supersedes:** [ADR-008](ADR-008-split-pane-editor.md) — WebSocket transport and the dual conversation/generation-mode split for v1.0. ADR-008's product vision (conversational split-pane editor with durable history) remains the long-term target; this ADR records the v1.0 delivery shape.
**Deciders:** Gabriel Specian (Nexian Tech)

---

## Context

ADR-008 specified a WebSocket-connected split-pane editor with two explicit modes (conversation, generation), inline section click-to-edit, and Chronik-backed durable conversation history. During Sprints 4–8 the editor shipped on **Server-Sent Events (SSE)** instead of WebSocket, as a single chat-style stream without the mode toggle or section click-to-edit. The [v1.0 code review](../code-reviews/code-review-v1.0-readiness.md) finding 4.3 flagged this as a contract gap.

Sprint 10 is the v1.0 release sprint. Rebuilding the editor transport and rewriting the AI system prompt to distinguish modes is scoped at ~3 story points — work that is feasible but carries release-slip risk against a 16-pt sprint already full of P0 hardening. The sprint risk register explicitly calls out ADR-008 as a Day 1 reaffirm-or-supersede decision.

This ADR records the decision: **SSE is the intentional transport for v1.0.**

---

## Decision

For **v1.0 only**:

- The editor transport is **Server-Sent Events (SSE)**, not WebSocket.
- The editor is a **single conversational stream**, without the conversation/generation mode split.
- Drafts are **auto-saved** to PostgreSQL `page_versions` with debounced writes (2 s idle, 30 s max).
- Conversation history is persisted to a new PostgreSQL `editor_conversations` table keyed by `(page_id, language, user_id)`. Chronik topic `editor-conversations:stream` remains provisioned but the v1.0 write path is PostgreSQL-only.
- **Inline section click-to-edit is not shipped in v1.0.**

For **v1.1** (tracked as GitHub issue under the `v1.1.0` milestone):

- Rebuild the editor transport on WebSocket per ADR-008.
- Introduce the explicit `conversation` / `generation_chunk` / `generation_complete` / `ai_thinking` message envelope.
- Add section click-to-edit with `section_focus` messages.
- Dual-write conversation events to the `editor-conversations` Chronik topic alongside PostgreSQL for event-sourcing analytics downstream.

---

## Why

1. **SSE meets v1.0 user needs.** Token-streaming output and chat-style input together deliver the core author experience. The only capabilities SSE cannot carry that ADR-008 called for are bidirectional section-focus messages and the mode toggle — both refinements, not foundational.
2. **Release risk dominates.** Sprint 10 is the public-launch sprint. The MCP JSON-RPC wrapper (3 pt) and editor hardening (2 pt) already concentrate the release-blocking work; adding a full transport rewrite on top pushes the sprint over capacity.
3. **The data model is already forward-compatible.** `page_versions` exists and carries the auto-save target. The new `editor_conversations` table can be cleanly consumed by a future WebSocket implementation. The Chronik topic is already declared in `docker-compose.yml` — v1.1 will only change the write path, not the schema.
4. **Architectural reversibility is high.** Moving from SSE + REST to WebSocket is a localized change inside `apps/web/features/editor/` and the corresponding Axum handler. No persisted state or API contract outside the editor needs to change.

---

## Consequences

**Easier (v1.0):**
- Sprint 10 ships on time with auto-save + conversation persistence, closing the code review's 4.3 "minimum bar" without the full ADR-008 rebuild.
- The CHANGELOG's "Known Limitations" section honestly documents the deviation and the v1.1 path.
- Existing SSE infrastructure (`apps/web/features/editor/`, `apps/api/src/application/editor/`) continues to work without rewrite.

**Harder (v1.0 → v1.1):**
- Claude Desktop users and other MCP clients that read the editor's protocol hints will need to rediscover capabilities after the v1.1 upgrade. Mitigation: document the transport change prominently in the v1.1 CHANGELOG.
- The mode toggle's absence means authors cannot explicitly flip the AI between conversation and generation — the v1.0 AI system prompt must handle both implicitly. Mitigation: keep the current system prompt; do not over-invest in prompt engineering that will be redone for v1.1.
- Dual-writing conversations to Chronik later means back-filling historical conversations or accepting a cut-over date. Mitigation: note the cut-over date in v1.1 release notes.

**Must revisit at v1.1:**
- Re-read ADR-008 before planning the WebSocket rebuild. Evaluate whether the mode toggle remains the right UX, or whether usage data from v1.0 suggests a different control surface.
- Decide whether v1.1 introduces section click-to-edit as part of the WebSocket rebuild, or as a separate increment.

---

## Action Items

1. [x] Sprint 10 item #4 stays scoped as auto-save + `editor_conversations` persistence over SSE
2. [ ] File a v1.1 GitHub issue titled "Rebuild editor on WebSocket per ADR-008" with the mode-toggle and section click-to-edit work as subtasks, under the `v1.1.0` milestone, before tagging `v1.0.0`
3. [ ] Update `CHANGELOG.md`'s `v1.0.0` "Known Limitations" section to declare the SSE transport and cite this ADR
4. [ ] Update README's MCP protocol compliance section to note the editor uses SSE and link this ADR for the rationale
