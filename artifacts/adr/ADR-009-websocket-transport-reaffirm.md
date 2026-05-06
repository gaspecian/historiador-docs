# ADR-009: Reaffirm WebSocket Transport for the Editor — Close the SSE Deviation

**Status:** Accepted
**Date:** 2026-04-19
**Extends:** [ADR-008](ADR-008-split-pane-editor.md)
**Deciders:** Gabriel Specian (Nexian Tech)

---

## Context

ADR-008 specified a WebSocket between the Next.js frontend and the Axum API server for the editor. What actually shipped in Sprint 4 was Server-Sent Events — a one-way stream from server to client, with author messages sent over separate HTTP POSTs. The code review documented in `code-review-v1.0-readiness.md` (2026-04-18) flagged this as a silent contract violation against ADR-008. Sprint 10 kept SSE intentionally to ship `v1.0.0` quickly and explicitly deferred the WebSocket rebuild to v1.1.

Sprint 11 is the v1.1 rebuild. The SSE-plus-POST shape is no longer adequate because Sprint 11 introduces:

- Block-level operations from the AI that the frontend must acknowledge (ADR-012, ADR-013)
- Tool calls that flow back and forth between the editor and the LLM (ADR-011)
- Autonomy checkpoints that pause the AI mid-generation and wait for an author decision (ADR-014)
- Inline comments that the author posts up the same channel as conversation messages (ADR-016)

All of these require **bidirectional, ordered, low-latency** traffic on a single channel. SSE-plus-POST is bidirectional only in the loosest sense: the down-stream is one channel, every up-stream is a separate HTTP request, and stitching the two together is an application-level chore.

Three transports were on the table: keep SSE, move to WebSocket, or move to gRPC/Connect. gRPC/Connect was discussed and declined for Sprint 11 (see project memory `project_editor_transport.md`) — the toolchain commitment isn't justified by the editor alone, and the MCP server is already settling on JSON-RPC over HTTP.

---

## Decision

**Reaffirm ADR-008's WebSocket choice.** The Sprint 11 editor is a ground-up replacement of the SSE layer, not an incremental patch. The new transport:

- Single persistent WebSocket per open editor tab, keyed by `(page_id, author_id)`
- Heartbeat every 15 seconds; reconnect with exponential backoff on drop
- Message ordering guaranteed by a monotonic `seq` field on every envelope message
- **All** editor traffic — author messages, AI conversation, generation chunks, block ops, tool calls, autonomy decisions, inline comments — rides the same socket using the extended `EditorMessage` union (ADR-012)
- On reconnect, the server replays missing events from the `editor-conversations` Chronik topic since the last client-acknowledged `seq`

ADR-008's envelope shape is preserved; the union is **extended** in ADR-012, not replaced.

---

## Options Considered

### Option A: WebSocket (rebuild) ✅ Selected

| Dimension | Assessment |
|-----------|------------|
| Bidirectional traffic | Native — one channel, both directions |
| Sprint 11 protocol fit | Excellent — all variants ride one wire |
| Reconnect / replay | Clean — `seq` + `editor-conversations` event log |
| Operational complexity | Medium — reconnect logic, reverse-proxy WebSocket support required |
| Deployment surface | Single port, no proxy translation layer |

**Pros:**
- Restores the contract documented in ADR-008
- Bidirectional flows (section focus, block-op acknowledgements, autonomy checkpoints) become natural
- Sprint 4's SSE glue code is deleted in full — no parallel POST channel to maintain
- The same envelope works for every Sprint 11 feature; no per-feature transport choices

**Cons:**
- Connection lifecycle adds code: heartbeat, reconnect, replay reconciliation
- Network flakiness becomes a product-visible problem and must be designed for
- Reverse proxies must be configured for WebSocket upgrade (nginx, Traefik, Cloudflare) — installation guide must cover this

### Option B: Stay on SSE + parallel POSTs (status quo)

The current shipped state. Server-Sent Events stream from the API to the frontend; author messages and acknowledgements go through separate HTTP POST endpoints.

**Pros:**
- Already works; nothing to rebuild
- HTTP-only; no special proxy configuration

**Cons:**
- Cannot cleanly support correlated request/response patterns (block-op proposals + acknowledgements, tool calls + results) without significant application-level orchestration
- Browser SSE connection limit (~6 per origin) silently breaks when an author opens multiple editor tabs
- Ordering guarantees between the SSE stream and parallel POSTs require careful sequencing — exactly the complexity Sprint 11 multiplies

**Rejected** — fails the bidirectional and ordering requirements that drive Sprint 11.

### Option C: gRPC / Connect

Bidirectional streaming via Connect-RPC (gRPC-Web has no real bidi streaming in browsers).

**Pros:**
- Strong wire-level typing via Protocol Buffers
- Same protocol for editor and (eventually) API/MCP services

**Cons:**
- Requires either an Envoy proxy or `tonic-web` middleware in the Rust server
- Replaces OpenAPI codegen (ADR-006) with protobuf codegen — a toolchain swap that affects more than the editor
- The MCP server is already on JSON-RPC over HTTP; introducing Connect for the editor alone creates stack asymmetry
- Browser-side Connect tooling is younger than WebSocket tooling

**Rejected for Sprint 11** — defensible only if we'd also migrate the API and MCP services. See `project_editor_transport.md`.

---

## Architecture

### Connection lifecycle

```
Client opens tab
    → WS connect to /editor/ws?page_id=…
        → Server checks auth, opens session, assigns next seq
            → Heartbeat ping/pong every 15s
                → If pong missed twice (30s): client reconnects
                    → On reconnect, client sends last_acked_seq
                        → Server replays editor-conversations events with seq > last_acked_seq
                            → Steady state resumes
```

### Message ordering

Every server-emitted message carries `seq: u64`, monotonically increasing per `(page_id, author_id)` session. Client-emitted messages do not carry `seq` — the server assigns one when it persists them to `editor-conversations`. The client tracks the highest `seq` it has applied; on reconnect it sends this as `last_acked_seq` and the server replays any later events.

### Replay-on-reconnect

The `editor-conversations` Chronik topic (ADR-007) is the durable event log. On reconnect, the server queries events for the page since `last_acked_seq` and re-emits them through the new socket. This means a brief network drop never loses an in-flight AI generation or a pending block-op proposal — they replay deterministically.

### Per-tab vs per-page

One socket per `(page_id, author_id, tab)`. The same author opening two tabs on the same page gets two sockets. Both subscribe to the same underlying `editor-conversations` partition and see each other's edits.

---

## Consequences

**Easier:**
- The wire protocol is one thing — every Sprint 11 feature uses the same envelope
- Bidirectional flows are first-class; no parallel-POST orchestration
- Reconnect/replay is a property of the architecture, not a feature to bolt on later
- The SSE codepath (`crates/api/src/editor_sse.rs` and matching client code) is deleted, reducing surface area

**Harder:**
- Reverse proxies in the user's deployment must support WebSocket upgrade — installation guide must call this out explicitly
- Connection lifecycle introduces failure modes (drop, slow reconnect, replay race) that didn't exist with stateless POSTs
- Local development must handle WebSocket reload — `next dev` and `cargo watch` both need to be configured to handle hot-reload without dropping editor sessions

**Must revisit:**
- If WebSocket termination turns out to be unreliable behind the chosen reverse proxy (rare but possible with some Cloudflare configurations or older nginx), fall back to long-polling using the same envelope rather than returning to SSE
- If multi-author real-time collaboration ever lands (post-v1.1), the per-tab socket model already supports it; broadcasting becomes a server-side fanout rather than a transport change

---

## Action Items

1. [ ] Delete the SSE handler in `crates/api/src/editor_sse.rs` and the corresponding client code
2. [ ] Implement `crates/api/src/editor_ws.rs` with the ADR-008 envelope plus the ADR-012 extensions
3. [ ] Implement replay-on-reconnect: query `editor-conversations` for events with `seq > last_acked_seq` and re-emit them through the new socket
4. [ ] Update `apps/web` editor to a single WebSocket client; remove the POST-and-subscribe pattern
5. [ ] Implement heartbeat (ping/pong every 15s, drop after 30s) with exponential backoff reconnect
6. [ ] Document reverse-proxy WebSocket upgrade configuration in the installation guide (nginx, Traefik examples)
7. [ ] Add `cargo watch` and `next dev` configuration notes for keeping editor sessions alive across hot-reloads
