# ADR-012: Extended EditorMessage Envelope for Block Ops, Tool Calls, Autonomy, and Comments

**Status:** Accepted
**Date:** 2026-04-19
**Extends:** [ADR-008](ADR-008-split-pane-editor.md), [ADR-009](ADR-009-websocket-transport-reaffirm.md), [ADR-010](ADR-010-canvas-block-tree.md), [ADR-011](ADR-011-llm-tool-calling.md)
**Deciders:** Gabriel Specian (Nexian Tech)

---

## Context

ADR-008 defined the `EditorMessage` envelope with four variants: `conversation`, `generation_chunk`, `generation_complete`, `ai_thinking`. These covered the original split-pane editor model — the AI either talks (left pane) or writes content (right pane).

Sprint 11 introduces several flows the original envelope cannot express:

- **Block ops** — the AI emits targeted operations against the block tree (ADR-010), and the frontend must acknowledge each one (ADR-013)
- **Tool calls and results** — the LLM emits tool calls (ADR-011) that the editor handler executes and replies to
- **Autonomy checkpoints** — the AI pauses mid-generation and waits for an author decision (ADR-014)
- **Inline comments** — authors post comments that flow up the same channel as conversation messages (ADR-016)

Three options were on the table: extend the existing envelope, split traffic across multiple WebSockets, or version-bump with a breaking change.

---

## Decision

**Extend, don't replace.** All existing variants stay. The union grows to cover Sprint 11 flows. Every message carries a monotonic `seq` field to support the ADR-009 replay-on-reconnect contract.

```typescript
type EditorMessage =
  // Existing (ADR-008)
  | { type: 'conversation'; seq: number; content: string }
  | { type: 'generation_chunk'; seq: number; content: string }
  | { type: 'generation_complete'; seq: number; section: string }
  | { type: 'ai_thinking'; seq: number }

  // New (Sprint 11)
  | { type: 'block_op'; seq: number; op: BlockOp; proposal_id: string }
  | { type: 'block_op_ack'; seq: number; proposal_id: string;
      outcome: 'applied' | 'rejected' | 'edited' }
  | { type: 'tool_call'; seq: number; call_id: string; name: string; arguments: unknown }
  | { type: 'tool_result'; seq: number; call_id: string; result: unknown }
  | { type: 'autonomy_checkpoint'; seq: number; checkpoint_id: string; summary: string }
  | { type: 'autonomy_decision'; seq: number; checkpoint_id: string;
      decision: 'approve' | 'reject' | 'edit' }
  | { type: 'comment_posted'; seq: number; comment_id: string;
      anchor: { kind: 'blocks'; block_ids: string[] } | { kind: 'whole_page' };
      text: string }
  | { type: 'comment_resolved'; seq: number; comment_id: string };

type BlockOp =
  | { kind: 'insertBlock'; after_block_id: string | null; block: Block }
  | { kind: 'replaceBlock'; block_id: string; block: Block }
  | { kind: 'appendToSection'; section_heading: string; blocks: Block[] }
  | { kind: 'deleteBlock'; block_id: string }
  | { kind: 'suggestBlockChange'; block_id: string;
      proposed_block: Block; rationale: string };
```

**Backwards compatibility rule:** unknown variants are logged and silently dropped, never errored. A protocol-version negotiation handshake on connect declares which variants each side supports.

---

## Options Considered

### Option A: Single envelope, extended union ✅ Selected

| Dimension | Assessment |
|-----------|------------|
| Wire complexity | Low — one schema |
| Dispatch complexity | Low — one switch on `type` |
| Connection count | One per editor tab |
| Ordering | Trivial — single channel |
| Schema growth | Manageable up to ~15 variants; namespace split beyond that |

**Pros:**
- One wire protocol; one dispatch table on each side
- All Sprint 11 traffic ordered relative to each other (block ops never overtake the conversation message that triggered them)
- Reconnect/replay (ADR-009) is uniform across all message types
- New variants are added without negotiating new transports

**Cons:**
- The union grows; if it exceeds ~15 variants, dispatch code becomes unwieldy and should be split into namespaces (`editor.*`, `tool.*`, `autonomy.*`, `comment.*`)
- Type definitions in `packages/types` get long; codegen must keep up

### Option B: Multiple WebSockets per concern (block-ops socket, conversation socket, autonomy socket)

**Pros:**
- Clean separation of concerns at the transport level
- Each socket can be scaled / monitored independently

**Cons:**
- Connection count multiplies per editor tab
- Cross-channel ordering (conversation → block op → ack) becomes a synchronization problem
- Reconnect logic multiplies — every socket needs heartbeat, replay, backoff
- Browser connection-per-origin limits become a real constraint

**Rejected** — solves a problem we don't have at the cost of one we'd have to build.

### Option C: New protocol version with breaking change

Define `EditorMessage v2` with a redesigned shape; v1.0 clients are no longer supported.

**Pros:**
- Could simplify the schema if started from scratch

**Cons:**
- No v1.0 clients to break (the editor is rebuilt anyway in this sprint)
- Discards backwards-compatibility discipline for no real benefit

**Rejected** — extension achieves the same shape with no compatibility loss.

---

## Architecture

### Sequence numbering

`seq` is assigned by the server when persisting any message to `editor-conversations`. Client-emitted messages do not carry `seq`; the server stamps them on receipt. Client tracks the highest `seq` it has applied; on reconnect it replays from `last_acked_seq + 1`.

### Proposal correlation

Every `block_op` carries a `proposal_id`. The frontend's `block_op_ack` quotes the same `proposal_id`. This lets the AI handler track which proposals are still pending, which were approved (so it can continue building on them), and which were rejected (so it can re-plan).

### Tool-call correlation

`tool_call` and `tool_result` correlate via `call_id` (assigned by the LLM adapter, ADR-011). Tool results flow back to the LLM in the next request to complete the tool-use cycle.

### Autonomy checkpoints

The AI handler pauses generation when entering a checkpoint (Checkpointed mode, ADR-014), emits an `autonomy_checkpoint` summarizing the proposed batch, and waits for an `autonomy_decision`. Approval resumes generation; rejection rolls back the batch from the overlay.

### Backwards compatibility

On connect, both sides exchange a list of supported variants:

```
Client → Server: { hello: { client_version: "1.1.0", supported: ["conversation", "generation_chunk", ...] } }
Server → Client: { hello: { server_version: "1.1.0", supported: [...] } }
```

If the server emits a variant the client doesn't list, the client logs and drops it. If the client emits an unsupported variant, the server logs and ignores it. **Errors are not raised** — the rule is forward compatibility by default.

---

## Consequences

**Easier:**
- All editor traffic dispatches uniformly; one place to add a new feature variant
- Ordering is trivial; replay is uniform
- Negotiated compatibility means a v1.1 server can talk to a future v1.2 client and ignore variants it doesn't know

**Harder:**
- The union is large; type-definition discipline matters
- Backwards compatibility must be tested — adding a variant requires a "v1.1 server with v1.2 client" test case
- Dispatch on `type` becomes verbose; consider a generated dispatcher rather than hand-written switches

**Must revisit:**
- If the union exceeds ~15 variants, split into namespaces by prefix (`editor.conversation`, `tool.call`, etc.) and adjust dispatch
- Backwards-compatibility window for clients (how long do we guarantee a v1.0 client works against a v1.2 server?) is still open — see Sprint 11 v2 architecture doc

---

## Action Items

1. [ ] Define the extended `EditorMessage` union in `packages/types`, generated from OpenAPI
2. [ ] Implement dispatch in Rust (`crates/api`) with a shared enum of message types
3. [ ] Implement dispatch in TypeScript (`apps/web`) with the same enum
4. [ ] Implement the `hello` handshake with `supported[]` negotiation on connect
5. [ ] Implement the "log and drop unknown variants" rule on both sides
6. [ ] Integration tests: replay the full variant set across a round trip
7. [ ] Compatibility tests: server-with-extra-variant + client-without-variant scenario
8. [ ] Document the union and its variants in the developer docs
