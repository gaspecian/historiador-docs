# Historiador Doc — v2 Plan

**Status:** Post-v1.0 planning (not yet started)
**Prerequisite:** `v1.0.0` shipped and in use by at least one real team for 2–4 weeks
**Owner:** Gabriel Specian, Nexian Tech
**Last Updated:** 2026-04-12

> This document is a strategic plan, not a sprint plan. Sprint-level planning for v2 happens after `v1.0.0` ships and real usage data informs prioritization. The Chronik `mcp-queries` analytics from Sprint 7 are the primary input.

---

## Why v2 Exists

v1.0 solves the creation and retrieval problem. v2 solves the **discovery problem**: the system passively learns which questions employees are asking that documentation doesn't yet answer, and turns that signal into content creation prompts.

This transforms Historiador Doc from a documentation tool into a self-improving knowledge base — one that gets smarter the more it's used.

---

## v2 Feature Set

### 1. Gap Detection Flywheel (Highest Priority)

**What it is:** The system watches the stream of MCP queries that return zero results (or low-confidence results) and clusters them by topic. It surfaces these clusters as documentation requests — pre-filled briefs in the AI editor — so authors are always working on the documentation that users actually need.

**Why now:** The data infrastructure is already in place as of Sprint 7. Every MCP query is written to the Chronik `mcp-queries` topic. Gap detection is a DataFusion SQL query on top of data that already exists — it's not a new infrastructure build.

**Architecture (already designed in ADR-007):**

```
MCP queries → Chronik `mcp-queries` topic
                      ↓
             DataFusion SQL (scheduled, daily)
             SELECT query_text, COUNT(*) as frequency
             FROM mcp_queries
             WHERE result_count = 0
               AND timestamp > NOW() - INTERVAL 7 DAYS
             GROUP BY query_text
             ORDER BY frequency DESC
             LIMIT 50
                      ↓
             Topic clustering (simple keyword grouping or
             embedding-based cosine similarity clustering)
                      ↓
             `documentation_requests` table (PostgreSQL)
             { cluster_label, example_queries[], frequency,
               suggested_collection, status }
                      ↓
             "Documentation Requests" dashboard section
             → Click to open AI editor with pre-filled brief
```

**Sprint estimate:** 2 sprints (Sprint 10–11)

- **Sprint 10:** DataFusion gap query, clustering pipeline, `documentation_requests` table, background job (runs daily)
- **Sprint 11:** "Documentation Requests" section in dashboard, one-click "Create doc from request" flow, weekly digest email (optional)

**Success metric:** At least 30% of documentation requests surfaced by the flywheel result in a new published page within 7 days.

---

### 2. SSO / SAML Integration (High Priority)

**What it is:** IT admins can configure Okta, Azure AD, or Google Workspace as the identity provider. Users authenticate via SSO instead of email/password. Role mapping is configured via the IdP (e.g., group `historiador-admins` maps to Admin role).

**Why it matters:** Without SSO, Historiador Doc cannot be adopted by any enterprise that requires centralized identity management — which is most mid-to-large companies. This is a gate, not a nice-to-have, for serious enterprise adoption.

**Architecture notes:**
- Use `samael` (Rust SAML library) or an OAuth2 / OIDC flow for Google/Okta/Azure AD
- The `users` table already has `external_id` and `provider` columns (or should — add them in Sprint 10 if not)
- JWT session tokens remain unchanged; SSO replaces only the authentication step
- Role mapping: configurable via the admin panel; default is all SSO users get Viewer role unless mapped

**Sprint estimate:** 1 sprint (Sprint 12)

---

### 3. Multi-Workspace Support (Medium Priority)

**What it is:** A single Historiador Doc installation hosts multiple isolated workspaces. Each workspace has its own users, collections, pages, language configuration, MCP endpoint, and LLM API key. Useful for agencies, holding companies, and multi-brand organizations.

**Why it's medium (not high):** The v1.0 data model was designed for this — every table has a `workspace_id` foreign key. The feature is largely an admin UX and routing problem, not a data model change.

**Architecture notes:**
- The `workspace_id` FK is already on all tables — the data isolation model is in place
- Routing: each workspace gets a subdomain (`engineering.historiador.company.com`) or path prefix (`/workspaces/engineering`)
- The MCP endpoint URL becomes workspace-scoped: `/workspaces/:workspace_id/mcp/query`
- Billing/pricing model consideration: multi-workspace is the natural monetization boundary for a potential SaaS tier

**Sprint estimate:** 1 sprint (Sprint 13)

---

### 4. Webhook Notifications (Lower Priority)

**What it is:** On page publish or update, Historiador Doc can POST a notification to a configured webhook URL (Slack incoming webhook, custom endpoint, etc.). Payload: `{ event: 'page.published', page_id, title, collection_path, author, language, url }`.

**Why it matters:** Keeps teams informed without requiring everyone to check the dashboard. Also enables integration with external systems (triggering a CI pipeline when docs change, feeding a Slack `#docs-updates` channel).

**Architecture notes:**
- Webhook configuration in the admin panel: URL, events to subscribe to, optional secret for HMAC signature verification
- Fire-and-forget from a background task — webhook delivery failure must not block page publish
- Add retry logic: 3 attempts with exponential backoff; log failures to the admin panel

**Sprint estimate:** Part of a sprint (3 pts), likely bundled with another small feature

---

### 5. Public Documentation Site (Medium Priority)

**What it is:** Individual pages or collections can be marked "public." Public pages are accessible without authentication via a clean, branded URL (`docs.company.com`). The public site is generated from published pages and served statically or via a lightweight Axum handler.

**Why it matters:** Turns Historiador Doc into a full documentation publishing platform — internal knowledge base and external developer docs from a single system.

**Architecture notes:**
- This requires the "configurable public pages" P1 feature as a foundation (not yet built in v1.0)
- The MCP endpoint for public workspaces does not require a bearer token — or uses a separate public-read token
- Custom domain support: wildcard DNS pointing to the Historiador Doc instance; host header routing to the right workspace

**Sprint estimate:** 2 sprints (Sprint 14–15)

---

## v2 Milestone Dependencies

```
v1.0.0 shipped
      │
      ├── 4 weeks of real usage data from Chronik mcp-queries
      │         ↓
      │   Sprint 10-11: Gap Detection Flywheel
      │
      ├── Sprint 12: SSO/SAML
      │
      ├── Sprint 13: Multi-Workspace
      │
      └── Sprint 14-15: Public Docs Site (requires "public pages" P1 first)
```

Webhook notifications can be bundled with any sprint that has spare capacity.

---

## v2 Success Metrics

| Feature | Success Signal |
|---------|---------------|
| Gap detection flywheel | ≥ 30% of surfaced requests result in a published page within 7 days |
| SSO | At least 1 enterprise customer can complete SSO setup in < 2 hours from the admin panel |
| Multi-workspace | A single installation runs 3+ isolated workspaces without performance degradation |
| Public docs site | An external user can access public pages via a custom domain without authentication |

---

## What to Do Before v2 Planning Begins

1. **Collect real usage data** — run Historiador Doc internally at Nexian Tech for 2–4 weeks post-v1.0. Let the `mcp-queries` topic fill with real queries.
2. **Read the analytics** — use the Sprint 7 admin analytics dashboard to identify the actual no-result query patterns. Let real data, not assumptions, drive gap detection design decisions.
3. **File v2 issues on GitHub** — translate each v2 feature into one or more GitHub issues before writing a sprint plan. Community upvotes on issues are a signal for prioritization.
4. **Confirm Chronik-Stream DataFusion API** — the gap detection flywheel depends on running arbitrary DataFusion SQL against the `mcp-queries` topic. Validate this capability with the Chronik team before committing to Sprint 10 scope.

---

## v2 Candidate Sprints (Indicative — Not Committed)

| Sprint | Feature | Estimate |
|--------|---------|---------|
| Sprint 10 | Gap detection — data pipeline (DataFusion query + clustering + `documentation_requests` table) | 8 pts |
| Sprint 11 | Gap detection — dashboard UX + one-click doc creation + digest | 8 pts |
| Sprint 12 | SSO / SAML (Okta, Azure AD, Google Workspace) | 8 pts |
| Sprint 13 | Multi-workspace support | 8 pts |
| Sprint 14 | Public pages (configurable visibility) | 8 pts |
| Sprint 15 | Public documentation site (branded portal + custom domain) | 8 pts |

**Estimated v2 timeline:** 6 weeks post-v1.0 for gap detection + SSO + multi-workspace. Public docs site adds 2 more weeks.
