# Sprint 7 — Page Version History + MCP Usage Analytics

**Dates:** Week 7 (5 working days)
**Team:** Gabriel Specian (solo)
**Sprint Goal:** Authors can view and restore any previous version of a page, and the IT admin can see basic MCP query metrics — total queries, top topics, and queries that returned no results — from the admin panel.

---

## Context

Both items in this sprint are P1 features with specific audiences:

- **Page version history** is the primary ask from compliance-sensitive authors (HR, legal). They need an audit trail before they'll trust the system with policy documentation. It's also a natural safety net for the AI editor — if the AI produces a bad draft, authors can roll back.
- **MCP usage analytics** is the precursor to the gap detection flywheel (v2). The data infrastructure (Chronik `mcp-queries` topic) was already designed in Sprint 4 — this sprint surfaces it in the admin panel. Even basic metrics (query count, no-result rate) give IT admins confidence that the MCP endpoint is being used correctly and give the product team signal for v2 prioritization.

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
| 1 | **Version storage on publish** | 1 pt | On every publish (and every auto-save), write the full page markdown content + metadata snapshot to `page_versions` as an immutable record. Fields: `version_id`, `page_id`, `language`, `content_markdown`, `authored_by`, `created_at`, `is_published`. The current published version is the latest `is_published = true` record per language. |
| 2 | **Version history API** | 1 pt | `GET /pages/:id/versions?language=:lang` — returns a paginated list of versions (id, authored_by, created_at, is_published, content_preview first 200 chars). `GET /pages/:id/versions/:version_id` — returns the full content of a specific version. `POST /pages/:id/versions/:version_id/restore` — copies the versioned content into a new draft (does not overwrite the published version automatically). |
| 3 | **Version history UI** | 2 pts | A "History" panel accessible from the page editor (icon in the top bar). Shows a timeline list of versions with author, timestamp, and a "published" badge on the active published version. Clicking a version shows a diff view or a preview of that version's markdown in the right pane. A "Restore as draft" button creates a new draft from that version. No inline diff required for v1 — a before/after preview is sufficient. |
| 4 | **MCP query logging to Chronik** | 1 pt | Every `POST /query` request to the MCP server writes an event to the Chronik `mcp-queries` topic: `{ query_text, timestamp, workspace_id, result_count, top_chunk_score, response_time_ms }`. If a result is returned, `result_count > 0`; otherwise `result_count = 0`. This is the data foundation for v2 gap detection. The write is fire-and-forget — it must not block the MCP query response. |
| 5 | **MCP analytics dashboard (admin panel)** | 3 pts | New "Analytics" section in the admin panel. Powered by DataFusion SQL queries against the Chronik `mcp-queries` topic. Metrics displayed: (a) total queries last 7 days / 30 days, (b) query volume chart (bar chart by day), (c) top 10 most frequent query topics (simple keyword clustering or top query_text frequency), (d) queries with zero results — count and the actual query texts. No charts library required; an HTML table is acceptable for v1. |

**Planned: 8 pts (80% capacity)**

---

## Stretch (2 pts)

| Item | Points | Notes |
|------|--------|-------|
| Version diff view | 1 pt | Side-by-side markdown diff between any two versions in the history panel. Use a diff library (e.g., `diff-match-patch`) — do not build the diff algorithm from scratch. |
| Analytics CSV export | 1 pt | Admin can download the MCP query log as a CSV for offline analysis. Useful for teams that want to route the data into their own BI tool. |

---

## Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| `page_versions` table growing unbounded | At high save frequency, version storage could become significant | Add a configurable retention policy (default: keep 50 versions per page). Implement a background job that prunes old non-published versions. The job can be a simple `sqlx` query run on a timer — does not need to be a full task queue. |
| DataFusion SQL query performance on `mcp-queries` topic | If the topic has millions of events, unindexed queries could be slow | Scope the analytics queries to a rolling 30-day window. Add a `timestamp` filter to every DataFusion query so the full topic is never scanned. |
| Version restore UX confusion | Authors may expect "restore" to immediately publish, overwriting the live page | Make the restore flow explicit: restore always creates a new draft, never overwrites a published page. Use the label "Restore as draft" consistently — not "Restore" alone. |

---

## Definition of Done

- [ ] Every publish and auto-save writes an immutable record to `page_versions`
- [ ] `GET /pages/:id/versions` returns a paginated version list; `POST .../restore` creates a draft correctly
- [ ] The History panel in the editor shows a version timeline; clicking any version previews its content in the right pane
- [ ] "Restore as draft" creates a new draft from any historical version without touching the published version
- [ ] Every MCP query writes a `{ query_text, result_count, response_time_ms, timestamp }` event to the Chronik `mcp-queries` topic asynchronously
- [ ] The admin panel Analytics section shows: 7-day and 30-day query totals, query volume by day, top query topics, and zero-result queries
- [ ] Analytics queries are scoped to a 30-day rolling window; no full-topic scans
- [ ] CI green

---

## Key Dates

| Date | Event |
|------|-------|
| Monday | Sprint start — version storage model + `page_versions` migration |
| Tuesday | Version history API endpoints |
| Wednesday | Version history UI (History panel + preview) |
| Thursday | MCP query logging to Chronik |
| Friday EOD | Admin analytics dashboard. End-to-end test: write 5 MCP queries, verify they appear in the analytics panel. Retro. |
