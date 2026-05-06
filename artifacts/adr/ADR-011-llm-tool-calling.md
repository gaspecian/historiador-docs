# ADR-011: LLM Tool-Calling Inside the LlmClient Trait

**Status:** Accepted
**Date:** 2026-04-19
**Extends:** [ADR-006](ADR-006-application-stack-rust.md)
**Deciders:** Gabriel Specian (Nexian Tech)

---

## Context

Sprint 11 needs the LLM to emit structured block operations (`insertBlock`, `replaceBlock`, `appendToSection`, `deleteBlock`, `suggestBlockChange`) rather than free-form markdown. This is the mechanism by which the AI proposes targeted changes to the canvas (ADR-010) and authors approve or reject them through the proposal overlay (ADR-013).

Every supported provider has a tool-calling API with a different shape:

- **OpenAI** — `tool_choice` parameter, `tools[]` with JSON Schema, `tool_calls[]` array in responses, parallel calls supported
- **Anthropic** — `tool_use` content blocks, input schemas in a slightly different format, single-shot per turn historically (parallel support is improving)
- **Ollama** — tool calls supported on recent versions but behavior varies by underlying model

ADR-006 already specifies an `LlmClient` trait in `crates/llm` that abstracts text generation across providers. The question is whether tool-calling lives behind the same abstraction, or whether each caller writes provider-specific code.

---

## Decision

**Extend the existing `LlmClient` trait with a provider-agnostic tool-calling interface.** Each provider adapter (`OpenAiClient`, `AnthropicClient`, `OllamaClient`) implements the translation into and out of provider-native formats. Callers — the editor conversation handler, future autonomy handlers, the comment-response handler — depend only on the trait.

```rust
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,  // JSON Schema
}

pub struct ToolCall {
    pub call_id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

pub enum LlmOutput {
    Text(String),
    ToolCalls(Vec<ToolCall>),  // for providers that batch
    Mixed { text: String, tool_calls: Vec<ToolCall> },
}

#[async_trait]
pub trait LlmClient {
    async fn generate(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<ToolSpec>>,
    ) -> Result<LlmOutput>;

    async fn generate_stream(
        &self,
        messages: Vec<Message>,
        tools: Option<Vec<ToolSpec>>,
    ) -> Result<impl Stream<Item = LlmChunk>>;
}
```

The streaming variant emits text chunks token-by-token but **buffers tool-call arguments inside the adapter** and surfaces a tool call only once the JSON is fully formed. This is conservative — it loses the ability to show partial tool-call text — but avoids the headache of partial JSON parsing across providers that all buffer differently.

The Sprint 11 block-op tools (`insertBlock`, `replaceBlock`, etc.) live in a new `crates/tools` crate as `ToolSpec` definitions with exhaustive JSON Schemas and Rust validation, shared between the editor handler and the LLM call sites.

---

## Options Considered

### Option A: Provider-agnostic interface in the LlmClient trait ✅ Selected

| Dimension | Assessment |
|-----------|------------|
| Caller complexity | Low — depends on trait only |
| Adapter complexity | Medium — translation per provider |
| Provider swap | Configuration change, no app code change |
| Future providers (Bedrock, Gemini) | Adapter-level addition |
| Streaming partial tool calls | Buffered, conservative |

**Pros:**
- Swapping providers is adapter-level work; supporting Bedrock or Gemini in v2 means writing one adapter
- Tool specs are defined once in `crates/tools` and shared by every caller
- Application code never sees provider-specific quirks (parallel tool calls, content-block shapes, JSON-vs-XML arguments)
- Aligns with ADR-006's intent of treating LLM choice as a configuration concern

**Cons:**
- Streaming tool-call arguments token-by-token is sacrificed in v1.1 (must buffer until the JSON is closed) — acceptable cost
- The trait surface grows; adapter authors must implement two methods correctly

### Option B: Pick OpenAI's tool-call format as canonical, translate inward in adapters

Define `ToolCall` to match OpenAI's wire format exactly; Anthropic and Ollama adapters translate from their native formats into the OpenAI shape.

**Pros:**
- One less abstraction layer

**Cons:**
- Couples our internal API permanently to OpenAI's design choices
- If OpenAI changes its tool-call shape (which it has, more than once), every other adapter has to re-translate to match
- The internal `ToolCall` struct is a leaky abstraction — code that touches it has to think about OpenAI semantics

**Rejected** — provider lock-in by API shape is exactly what the trait abstraction exists to prevent.

### Option C: Provider-specific code in callers (no abstraction)

Editor handler imports `async-openai` directly when configured for OpenAI; imports the Anthropic crate when configured for Anthropic.

**Pros:**
- Maximum flexibility per provider

**Cons:**
- Defeats the purpose of `LlmClient`; ADR-006's provider abstraction collapses
- Every caller must implement provider switching
- Tool spec definitions duplicate across call sites

**Rejected** — direct violation of ADR-006.

---

## Architecture

### Tool spec definition (in `crates/tools`)

```rust
pub fn block_op_tools() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: "insertBlock".into(),
            description: "Insert a new block after the specified block ID, or at the start if after_block_id is null.".into(),
            input_schema: json!({
                "type": "object",
                "required": ["block"],
                "properties": {
                    "after_block_id": { "type": ["string", "null"] },
                    "block": { "$ref": "#/definitions/Block" }
                },
                "definitions": { "Block": block_json_schema() }
            }),
        },
        // replaceBlock, appendToSection, deleteBlock, suggestBlockChange
    ]
}
```

These specs are sent to the LLM with every generation-mode request. Tool-call results come back through `LlmOutput::ToolCalls` and are dispatched to the block-op handler that constructs `BlockOp` envelope messages (ADR-012).

### Streaming with tools

```
Caller: generate_stream(messages, Some(tools))
    → adapter buffers provider stream
        → text tokens emitted as LlmChunk::Text
            → tool-call openings detected, JSON buffer started
                → JSON closed (e.g., closing brace of arguments object)
                    → emit LlmChunk::ToolCall with full arguments
                        → continue streaming
```

Partial tool-call JSON is **never** surfaced to the caller — the adapter waits until the JSON object is closeable.

---

## Consequences

**Easier:**
- Provider swap is configuration only
- Tool specs are shared across callers via `crates/tools`
- New tool definitions can be added without touching adapter code
- Future providers (Bedrock, Gemini) need only an adapter

**Harder:**
- Adapter authors must handle three different streaming behaviors and translate them into one consistent output
- The conservative streaming model (buffered tool calls) means very long tool arguments (e.g., a `replaceBlock` with a multi-paragraph body) appear as one big chunk rather than streaming
- Ollama tool-call support is uneven across models; the adapter must handle "model doesn't support tools" gracefully — likely by falling back to plain text generation

**Must revisit:**
- If buffered tool-call streaming creates a visibly slow UI for long block ops, extend `LlmChunk` to carry partial tool-call text with a `call_id` so the UI can show "AI is writing block X…" before the JSON is closed
- If a provider adds parallel-streaming tool calls (multiple tools open simultaneously), the adapter buffer becomes a per-`call_id` map — straightforward extension
- Ollama support quality should be benchmarked before promoting it to a first-class provider in marketing

---

## Action Items

1. [ ] Add `ToolSpec`, `ToolCall`, `LlmOutput`, and the streaming variant types to `crates/llm`
2. [ ] Implement tool-calling translation in the OpenAI adapter using `async-openai`
3. [ ] Implement tool-calling translation in the Anthropic adapter using `reqwest` + manual serialization
4. [ ] Stub Ollama tool-calling support; document model-compatibility caveats
5. [ ] Create `crates/tools` with the Sprint 11 block-op `ToolSpec` definitions
6. [ ] Define block-op argument validation in `crates/tools` (rejects malformed tool calls before they reach the dispatcher)
7. [ ] Adapter-level tests using recorded fixtures from real provider responses (no live API calls in CI)
8. [ ] Document the streaming tool-call buffering behavior so callers aren't surprised by it
