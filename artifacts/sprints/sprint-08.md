# Sprint 8 — Markdown Export + Ollama / Local LLM Support

**Dates:** Week 8 (5 working days)
**Team:** Gabriel Specian (solo)
**Sprint Goal:** Any admin can export the full knowledge base (or a single page) as organized markdown files, and IT can configure a local Ollama endpoint instead of a cloud API key — making Historiador Doc usable in air-gapped environments with no external API dependency.

---

## Context

Sprint 8 wraps the two remaining P1 features before the final Beta sprint:

- **Markdown export** is a trust and portability feature. Organizations evaluating Historiador Doc for long-term use need confidence they can get their data out. It's also the simplest backup mechanism available before a formal backup system exists.
- **Ollama support** unlocks a segment of high-value users that cannot use cloud LLM APIs: government agencies, financial institutions, healthcare organizations, and any team with strict data residency policies. The `LlmClient` trait in `crates/llm` was designed for this — Ollama is an implementation swap, not an architectural change.

These two items are independent of each other. If one stalls, the other can still ship.

---

## Capacity

| Person | Available Days | Allocation | Notes |
|--------|---------------|------------|-------|
| Gabriel | 5 of 5 | 8 pts committed / 2 stretch | |
| **Total** | **5** | **8 pts** | 1 point ≈ ~half a day |

---

## Sprint Backlog — P0

| # | Item | Points | Notes |
|---|------|--------|-------|
| 1 | **Markdown export API** | 1 pt | `GET /export` (admin only) — streams a `.zip` file containing all published pages as markdown files, organized by collection hierarchy (e.g., `Engineering/Backend/APIs.md`). The collection path becomes the directory structure. Each file includes a YAML front-matter block: `title`, `collection_path`, `language`, `last_updated`, `author`. `GET /pages/:id/export` — exports a single page as a markdown file (no zip). |
| 2 | **Export UI in admin panel** | 1 pt | "Export" button in the admin panel settings section. Triggers the full-workspace zip download. Shows a progress indicator while the zip is being built server-side. Also add a per-page "Download as Markdown" option in the page editor toolbar (three-dot menu). |
| 3 | **Ollama client in `crates/llm`** | 2 pts | Implement `OllamaClient` behind the `LlmClient` trait. Ollama's REST API is simple: `POST /api/generate` for text generation, `POST /api/embeddings` for embeddings. Streaming: Ollama returns newline-delimited JSON — parse the stream and pipe tokens to the WebSocket the same way the OpenAI/Anthropic clients do. The implementation should be testable with a local Ollama instance — add integration tests gated behind a `#[cfg(feature = "ollama-tests")]` flag. |
| 4 | **Ollama configuration in setup wizard + admin panel** | 1 pt | In the first-run setup wizard, add "Local Model (Ollama)" as a third provider option alongside OpenAI and Anthropic. When selected, show a single field: "Ollama base URL" (default: `http://localhost:11434`). Add a "Test connection" button that calls `GET /api/tags` on the Ollama endpoint and lists available models. Add a "Model" dropdown populated from the available models list. The same settings are editable from the admin panel LLM configuration section post-install. |
| 5 | **Embedding model configuration** | 1 pt | Currently, embedding generation is implicitly tied to the generation model. Separate the configuration: allow IT to choose a dedicated embedding model (e.g., `mxbai-embed-large` for Ollama, `text-embedding-3-small` for OpenAI). Store both `generation_model` and `embedding_model` in workspace settings. The chunker's embedding call uses `embedding_model`; the AI editor uses `generation_model`. |
| 6 | **Ollama documentation** | 2 pts | Write `docs/ollama-setup.md` covering: (a) installing Ollama on the same host as the Docker Compose stack, (b) pulling a recommended model (`llama3`, `mistral`), (c) pulling a recommended embedding model (`nomic-embed-text`), (d) network considerations when Ollama runs on the host and Historiador Doc runs in Docker (`host.docker.internal`), (e) known limitations vs. cloud LLM (no function calling, prompt format differences). Tested by completing setup on a fresh machine following only the doc. |

**Planned: 8 pts (80% capacity)**

---

## Stretch (2 pts)

| Item | Points | Notes |
|------|--------|-------|
| Collection-scoped export | 1 pt | Admin can export a single collection (and all its sub-collections) as a zip instead of always exporting the full workspace. Useful for teams that want to share a subset of docs with an external party. |
| Ollama model performance note in UI | 1 pt | When Ollama is configured, show a dismissable info banner in the AI editor: "You're using a local model. Generation may be slower than cloud APIs. Quality depends on the model you've selected." Link to the Ollama setup doc. |

---

## Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| Zip streaming for large workspaces | Building a zip of thousands of pages in memory before sending could OOM the API server | Use a streaming zip library that writes chunks to the response as pages are fetched — do not buffer the entire zip in memory. Evaluate `async-zip` for Rust streaming zip generation. |
| Ollama embedding dimensions differ from OpenAI | If workspace was initially configured with OpenAI embeddings and admin switches to Ollama, existing chunk embeddings are incompatible with new query embeddings | Block model switching once embeddings exist for the workspace. Show a warning: "Changing the embedding model requires re-indexing all published pages. This will take several minutes." Add a re-indexing trigger that re-embeds all published pages with the new model. |
| `host.docker.internal` not available on Linux Docker | Ollama on host + Historiador Doc in Docker — Linux doesn't have `host.docker.internal` by default | Document the Linux workaround (`--add-host=host.docker.internal:host-gateway` in Docker Compose) in the Ollama setup doc. |

---

## Definition of Done

- [ ] `GET /export` streams a zip of all published pages, organized by collection hierarchy, with YAML front-matter
- [ ] Admin can trigger a full-workspace export from the admin panel with one click
- [ ] Ollama is selectable as a provider in the setup wizard and admin panel
- [ ] The "Test connection" button in Ollama configuration verifies the Ollama URL and lists available models
- [ ] `OllamaClient` implements the full `LlmClient` trait including streaming text generation and embeddings
- [ ] Switching the embedding model warns the admin and offers to re-index all published pages
- [ ] `docs/ollama-setup.md` exists and has been tested on a fresh machine
- [ ] The chunker and AI editor use separate `embedding_model` and `generation_model` settings from workspace config
- [ ] CI green

---

## Key Dates

| Date | Event |
|------|-------|
| Monday | Sprint start — markdown export API + zip streaming |
| Tuesday | Export UI + Ollama client implementation |
| Wednesday | Ollama configuration in setup wizard + admin panel |
| Thursday | Embedding model separation + re-indexing trigger |
| Friday EOD | Ollama documentation. End-to-end test with local Ollama: install → configure Ollama → write a page → publish → query MCP. Retro. |
