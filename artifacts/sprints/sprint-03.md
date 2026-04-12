# Sprint 3 — Content Layer

**Dates:** Week 3 (5 working days)
**Team:** Gabriel Specian (solo)
**Sprint Goal:** Authors can create, organize, and publish documentation pages via the API. Published pages are chunked and stored in VexFS, ready to be queried.

> ⚠️ **Prerequisite before this sprint starts:** Confirm VexFS Rust client status with the VexFS team. If the client doesn't exist, implement the `VectorStore` trait + in-memory stub on Day 1 so the sprint doesn't stall.

---

## Capacity

| Person | Available Days | Allocation | Notes |
|--------|---------------|------------|-------|
| Gabriel | 5 of 5 | 8 pts committed / 2 stretch | VexFS stub fallback planned if Rust client unavailable |
| **Total** | **5** | **8 pts** | 1 point ≈ ~half a day |

---

## Sprint Backlog — P0 (Must Ship)

| # | Item | Points | Notes |
|---|------|--------|-------|
| 1 | **Page + Collection CRUD API** | 3 pts | Nested collections (adjacency list model). `POST /collections`, `PATCH /collections/:id` (rename, move), `DELETE /collections/:id` (with cascade prompt). `POST /pages`, `GET /pages/:id`, `PATCH /pages/:id`, `POST /pages/:id/publish`, `POST /pages/:id/draft`. Pages include `page_versions` per language. |
| 2 | **Structure-aware chunker (`crates/chunker`)** | 3 pts | `comrak` AST parser. Heading-boundary splitting. Paragraph fallback for oversized sections. Code blocks and tables are atomic — never split. Output: `Vec<Chunk>` with `{ heading_path, content, token_count, language, page_id, section_index, oversized }`. Async pipeline — triggered on publish, does not block the HTTP response. |
| 3 | **VexFS client (`crates/db`)** | 2 pts | If Rust client confirmed: wire real client. If not: implement `VectorStore` trait + in-memory stub. Interface: `upsert_chunks(Vec<Chunk>)`, `search(query_embedding, filters, top_k) -> Vec<ScoredChunk>`, `delete_page_chunks(page_id)`. Embeddings generated via `crates/llm` using the workspace API key. |

**Planned: 8 pts (80% capacity)**

---

## Stretch (2 pts)

| Item | Points | Notes |
|------|--------|-------|
| Page search endpoint | 1 pt | `GET /pages?q=term` — PostgreSQL `tsvector` full-text search across titles and content. |
| Collection tree endpoint | 1 pt | `GET /collections/tree` — returns the full nested hierarchy as a tree. Used by the dashboard sidebar in Sprint 5. |

---

## Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| VexFS Rust client does not exist | Chunker can't write to VexFS — embedding pipeline stalls | Implement `VectorStore` trait + stub on Day 1. Pages publish successfully; embeddings stored in-memory and lost on restart. Real client swapped in without changing other code. |
| `comrak` heading traversal complexity | Chunker takes longer than 3 pts to implement | Time-box chunker to 2.5 days. If complex, ship a simplified version (split at H2 only) and refine in Sprint 4. Don't let the chunker block page publishing. |
| Embedding API latency on publish | Slow embedding calls make publish feel sluggish to authors | Fire-and-forget: publish saves markdown synchronously, spawns async task for chunking + embedding. Author sees "Published" immediately; MCP reflects new content within seconds. |

---

## Definition of Done

- [ ] Author can create a nested collection hierarchy 3 levels deep via the API
- [ ] Author can create a page in a collection, save a draft, and publish it
- [ ] Publishing a page triggers async chunking; chunks stored (VexFS or stub) within 5 seconds of publish
- [ ] Chunk metadata includes `heading_path`, `language`, `page_id`, `token_count`
- [ ] Code blocks in test pages are never split across chunks
- [ ] Unit tests cover: standard sections, nested headings, code blocks, flat pages, oversized sections
- [ ] CI green

---

## Key Dates

| Date | Event |
|------|-------|
| Monday | Sprint start — confirm VexFS client status; begin CRUD API |
| Wednesday | Mid-sprint: CRUD API complete; chunker in progress |
| Friday EOD | Sprint end — publish a test page via `curl`; verify chunks exist in VexFS or stub |
