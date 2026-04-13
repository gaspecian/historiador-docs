# Sprint 6 — Multilingual Enforcement + OpenAPI Type Pipeline

**Dates:** Week 6 (5 working days)
**Team:** Gabriel Specian (solo)
**Sprint Goal:** Documentation authors are guided to produce every required language version of a page, the editor enforces it before publish, and the frontend consumes auto-generated TypeScript types from the Rust API — no hand-maintained type files anywhere in the codebase.

---

## Context

Sprint 5 shipped the Alpha (`v0.1.0-alpha`). Beta begins here. The two items in this sprint are foundational for everything that follows:

- **Multilingual enforcement** is P0 for v1.0 — without it, the language configuration set by IT at install time has no actual effect in the UI.
- **OpenAPI type pipeline** was a Sprint 1 stretch that may not have been completed. If it was already done, repurpose those points toward multilingual enforcement depth. The pipeline must be confirmed and stable before Sprint 7 adds new API surfaces.

---

## Capacity

| Person | Available Days | Allocation | Notes |
|--------|---------------|------------|-------|
| Gabriel | 5 of 5 | 8 pts committed / 2 stretch | First Beta sprint — carry any Alpha debt as explicit items |
| **Total** | **5** | **8 pts** | 1 point ≈ ~half a day |

---

## Sprint Backlog — P0

| # | Item | Points | Notes |
|---|------|--------|-------|
| 1 | **OpenAPI type pipeline (confirm + stabilize)** | 1 pt | If the `utoipa` → `openapi.yaml` → `openapi-typescript` → `packages/types/generated/` pipeline was completed as Alpha stretch, validate it end-to-end and add it to CI so it breaks the build if the schema drifts. If it was not completed, build it now. No hand-maintained TypeScript API types anywhere after this sprint. |
| 2 | **`page_versions` language model** | 1 pt | Ensure the `page_versions` table has one record per language per page (BCP 47 `language` column). Write a migration if the schema is not already correct. Add a `GET /pages/:id/versions` endpoint that returns all language versions and their completeness state. |
| 3 | **Language tab UI in split-pane editor** | 2 pts | Add a language tab bar above the right pane — one tab per configured workspace language. Each tab shows the language name and a completeness badge (✅ draft/published, ⚠️ missing). Switching tabs switches the active `page_version` being authored. The AI editor receives the active language as context and generates content in that language. |
| 4 | **Pre-publish completeness check** | 1 pt | When the author clicks "Publish," check whether all required language versions have at least a draft. If any are missing, show a modal: "Missing: French, Portuguese. Publish anyway or go back?" Publish is not blocked, but the warning is mandatory before the first publish of a page with missing versions. |
| 5 | **Dashboard language completeness flags** | 1 pt | In the page list view, pages with missing required language versions show a ⚠️ badge with a tooltip listing the missing languages. Clicking the badge navigates to the editor with the first missing language tab active. |
| 6 | **MCP `language` filter** | 2 pts | The MCP `POST /query` endpoint already returns chunks with a `language` field. Add support for an optional `language` query parameter (BCP 47). When provided, filter Chronik `published-pages` results to chunks of that language only. When omitted, return the best-matching chunks regardless of language (current behavior). Update the MCP bearer token setup docs to document the new parameter. |

**Planned: 8 pts (80% capacity)**

---

## Stretch (2 pts)

| Item | Points | Notes |
|------|--------|-------|
| Language version copy | 1 pt | Author can click "Copy from [language]" in a missing language tab to pre-populate the editor with the content of an existing language version. The AI then translates/adapts it as the author iterates. |
| CI gate on OpenAPI drift | 1 pt | Add a CI step that runs `cargo build` (generating `openapi.yaml`) and then `openapi-typescript` and fails if the output `packages/types/generated/` differs from the committed version. Prevents frontend/backend type drift from ever silently entering the codebase. |

---

## Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| Language tab state and WebSocket session state conflict | Switching language tabs during an active generation stream could corrupt the draft | Lock language tab switching during active generation (disable tabs when `generation_chunk` messages are in flight). Unlock on `generation_complete`. |
| OpenAPI pipeline not completed in Alpha | Delays the frontend type safety story | Treat it as Day 1 of this sprint. If it takes more than 1 point, steal from the language copy stretch item. |
| MCP language filter changes the retrieval ranking | Adding a filter may produce zero results for queries in under-documented languages | Return a clear empty-result response with a `language_filter_applied: true` flag rather than silently falling back to unfiltered results. Document the behavior. |

---

## Definition of Done

- [ ] `utoipa` → `openapi.yaml` → `openapi-typescript` pipeline runs as part of `turbo build` and is gated in CI
- [ ] The split-pane editor shows one language tab per configured workspace language; each tab has a completeness badge
- [ ] Publishing a page with missing language versions shows a warning modal — publish is not silently blocked, but the author is always informed
- [ ] Pages with missing language versions are flagged in the dashboard with a ⚠️ badge
- [ ] `POST /query` on the MCP server accepts and respects an optional `language` filter parameter
- [ ] Each `page_version` record carries a BCP 47 `language` column; the migration is applied cleanly on top of the Alpha schema
- [ ] No hand-maintained TypeScript type files remain in `apps/web/` — all API types come from `packages/types/generated/`
- [ ] CI green across all crates and the Next.js app

---

## Key Dates

| Date | Event |
|------|-------|
| Monday | Sprint start — confirm OpenAPI pipeline; `page_versions` migration |
| Tuesday | Language tab UI in split-pane editor |
| Wednesday | Pre-publish completeness check + dashboard flags |
| Thursday | MCP `language` filter |
| Friday EOD | End-to-end multilingual test: configure two languages, write page in both, query MCP with language filter. Retro. |
