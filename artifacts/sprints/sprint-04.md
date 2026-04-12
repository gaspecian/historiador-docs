# Sprint 4 — AI Editor + MCP Server

**Dates:** Week 4 (5 working days)
**Team:** Gabriel Specian (solo)
**Sprint Goal:** An author can describe a document in plain language and receive an AI-generated draft. A published page is queryable via the MCP endpoint and returns the correct chunk with source attribution.

---

## Capacity

| Person | Available Days | Allocation | Notes |
|--------|---------------|------------|-------|
| Gabriel | 5 of 5 | 8 pts committed / 2 stretch | |
| **Total** | **5** | **8 pts** | 1 point ≈ ~half a day |

---

## Sprint Backlog — P0 + P1

| # | Item | Points | Notes |
|---|------|--------|-------|
| 1 | **LLM integration (`crates/llm`)** | 2 pts | `LlmClient` trait: `generate_text(prompt) -> String`, `generate_embedding(text) -> Vec<f32>`. Implementations: `OpenAiClient` (`async-openai`), `AnthropicClient` (raw `reqwest`). Provider selected from workspace config. All calls use the workspace-level API key. |
| 2 | **AI editor backend** | 2 pts | `POST /editor/draft` — accepts a natural language brief, calls the LLM with a structured prompt, returns a markdown draft with headings and sections. `POST /editor/iterate` — accepts the current draft + a follow-up instruction, returns an updated draft. Prompts engineered to produce heading-rich, section-structured markdown. |
| 3 | **MCP server (`crates/mcp`)** | 3 pts | Standalone Axum service on port 3002. `POST /query` — accepts `{ query: string, language?: string, top_k?: number }`. Generates query embedding via `crates/llm`. Searches VexFS with optional language filter. Returns `{ chunks: [{ content, heading_path, page_title, collection_path, score, language }] }`. Bearer token auth middleware. Discrete response (no streaming). |
| 4 | **Next.js AI editor UI (basic)** | 1 pt | **P1.** Chat-style interface: text input for brief, AI response displayed as markdown preview. "Use this draft" button populates the page editor. Bare-bones — no styling. |

**Planned: 8 pts (80% capacity)**

---

## Stretch (2 pts)

| Item | Points | Notes |
|------|--------|-------|
| MCP compatibility test | 1 pt | Manually test the MCP endpoint against Claude Desktop or another MCP client. Verify response format matches the MCP protocol spec. |
| LLM prompt refinement | 1 pt | Test the editor prompt against 5 real documentation briefs. Refine the system prompt to improve heading structure and section quality. |

---

## Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| VexFS stub still in use | MCP queries work against in-memory data only — content lost on restart | Acceptable for Sprint 4 testing. Document clearly: "MCP content persists only while VexFS stub is running." |
| LLM prompt produces flat prose | No headings = chunker has nothing meaningful to split; retrieval quality degrades | Engineer system prompt to explicitly require H2/H3 headings. Test with 5 briefs before Sprint 5. Add post-processing step if needed. |
| MCP protocol spec compliance | MCP client rejects responses due to format mismatch | Read the MCP spec carefully before implementing. Test against the official MCP inspector tool if available. |
| Embedding dimension mismatch with VexFS | VexFS rejects embeddings of the wrong dimension | Confirm VexFS expected dimension with the VexFS team. Use `text-embedding-3-small` (1536 dims) as default; document the config option. |

---

## Definition of Done

- [ ] `POST /editor/draft` returns a structured markdown page with at least 2 headings for any non-trivial brief
- [ ] `POST /editor/iterate` updates the draft in response to a follow-up instruction
- [ ] `POST /query` on the MCP server returns the correct chunk with `page_title`, `heading_path`, and `collection_path`
- [ ] Bearer token on the MCP endpoint — requests without a valid token return 401
- [ ] MCP server and API server run as separate Docker containers; MCP server has no write path
- [ ] CI green on both `crates/api` and `crates/mcp`

---

## Key Dates

| Date | Event |
|------|-------|
| Monday | Sprint start — LLM client trait + OpenAI implementation |
| Tuesday | AI editor backend endpoints |
| Wednesday–Thursday | MCP server implementation |
| Friday EOD | Integration test: brief → draft → publish → MCP query returns correct chunk |
