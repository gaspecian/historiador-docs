# Sprint 5 — Dashboard & Alpha Release

**Dates:** Week 5 (5 working days)
**Team:** Gabriel Specian (solo)
**Sprint Goal:** The Historiador Doc dashboard is usable by a non-technical author. A new user can complete the full flow — install, set up, write a doc, publish it, and query it via Claude Desktop — without reading anything beyond the README.

---

## Capacity

| Person | Available Days | Allocation | Notes |
|--------|---------------|------------|-------|
| Gabriel | 5 of 5 | 8 pts committed / 2 stretch | Final sprint before Alpha milestone |
| **Total** | **5** | **8 pts** | 1 point ≈ ~half a day |

---

## Sprint Backlog — P0 (Must Ship)

| # | Item | Points | Notes |
|---|------|--------|-------|
| 1 | **Page management dashboard** | 3 pts | Next.js: nested collection tree sidebar (expand/collapse), page list per collection, create page via AI editor, draft/publish toggle, search bar (full-text). Language completeness badges (⚠️ missing language version). |
| 2 | **User management + admin panel** | 2 pts | User list with roles, invite user (copy activation link), deactivate user, display MCP endpoint URL + bearer token (with regenerate button), workspace language config display. |
| 3 | **End-to-end integration validation** | 1 pt | Full flow test: fresh Docker install → setup wizard → create collection → write page via AI editor → publish → confirm chunk in VexFS → query via MCP → correct answer returned. Document any friction points as Sprint 6 issues. |
| 4 | **Alpha README + setup documentation** | 1 pt | `README.md`: prerequisites, `docker compose up`, setup wizard walkthrough, connecting Claude Desktop to the MCP endpoint. Tested by completing the flow on a fresh machine without any prior knowledge. |
| 5 | **First-run setup wizard UI** | 1 pt | **P1.** Next.js wizard: LLM provider selection, API key input (with "test connection" button), language picker, admin account creation. Redirects to dashboard on success. |

**Planned: 8 pts (80% capacity)**

---

## Stretch (2 pts)

| Item | Points | Notes |
|------|--------|-------|
| MCP latency benchmark | 1 pt | Run 100 queries against VexFS with a realistic 5,000-chunk dataset. Record p50 and p95. Flag if p95 > 2s. |
| Sprint 6 backlog draft | 1 pt | Write Sprint 6 plan covering: multilingual enforcement, page version history, Ollama support, OpenAPI type pipeline. |

---

## Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| Next.js nested collection tree UI complexity | 3 points may not be enough built from scratch | Use a pre-built tree component (e.g., `react-arborist`) rather than building from scratch. Visual polish is not the goal — functionality is. |
| VexFS real client still not integrated | MCP works only against in-memory stub — content doesn't persist across restarts | Document as a known Alpha limitation. Add a prominent note in the README: "VexFS integration in progress — chunks persist only while the container is running." |
| README tested only by the author | Setup friction invisible until external testers try it | Ask the VexFS team to do a fresh install following only the README before the Alpha is shared more broadly. |

---

## Definition of Done

- [ ] A non-technical user can complete the full flow from fresh install to first MCP query using only the README
- [ ] The dashboard renders the nested collection tree with at least 2 levels of nesting
- [ ] Admin can invite a user, view the MCP endpoint URL, and regenerate the bearer token from the admin panel
- [ ] The setup wizard runs automatically on first `docker compose up` and redirects to the dashboard on completion
- [ ] End-to-end flow documented as a test script in `docs/alpha-validation.md`
- [ ] Alpha release tag pushed to GitHub (`v0.1.0-alpha`)
- [ ] Sprint 6 backlog drafted

---

## Key Dates

| Date | Event |
|------|-------|
| Monday | Sprint start — page management dashboard |
| Wednesday | Mid-sprint: admin panel + end-to-end validation |
| Thursday | README and setup documentation |
| Friday EOD | Alpha release tag (`v0.1.0-alpha`). Retro. Sprint 6 backlog drafted. |

---

## Alpha Milestone — Summary

| Sprint | Goal | Key Deliverable |
|--------|------|-----------------|
| **1** | Foundation | Monorepo + Docker Compose + DB schema + health check |
| **2** | Auth & Setup | First-run wizard + JWT auth + RBAC + user invite |
| **3** | Content Layer | Page/collection CRUD + chunker + VexFS integration |
| **4** | AI + MCP | AI editor backend + MCP server + LLM integration |
| **5** | Dashboard + Alpha | Next.js UI + admin panel + Alpha release (`v0.1.0-alpha`) |

**Total timeline:** 5 weeks from Sprint 1 start.
**Alpha definition:** Install → write doc → publish → query via MCP in under 60 minutes, on a fresh machine, from only the README.

---

## What Carries Into Beta (Sprint 6+)

Explicitly deferred from Alpha:

- Multilingual enforcement (language tabs in editor, completeness warnings)
- Page version history
- Ollama / local LLM support
- MCP usage analytics
- OpenAPI → TypeScript type pipeline (if not completed as stretch)
- Real VexFS Rust client (if still on stub)
- Performance validation at scale (p95 < 2s target)
- Public open-source release (`v1.0.0`)
