# ADR-003: MCP Server Deployment Architecture — Standalone Service

**Status:** Accepted
**Date:** 2026-04-08
**Deciders:** Gabriel Specian (Nexian Tech)

---

## Context

Historiador Doc exposes a Model Context Protocol (MCP) endpoint that external AI tools (Claude, Cursor, custom agents) use to query the company's knowledge base. A key product requirement is that companies can expose this endpoint externally (to allow AI tools outside their internal network to query it) while keeping the documentation authoring application and management API internal.

The MCP server must:
- Accept natural language queries and return relevant chunks from VexFS
- Be independently deployable and network-isolatable
- Have no write path — it is a pure read service
- Return discrete (non-streaming) responses in v1
- Support bearer token authentication
- Achieve < 2 second p95 response latency

The question is whether the MCP server should be implemented as a separate Node.js service or embedded within the main Historiador Doc API.

---

## Decision

**Deploy the MCP server as a standalone Node.js service**, separate from the main Historiador Doc API. It is included as a distinct container in the Docker Compose configuration and reads from VexFS and PostgreSQL but has no write access to either.

---

## Options Considered

### Option A: Standalone MCP Service ✅ Selected

| Dimension | Assessment |
|-----------|------------|
| Network isolation | Native — can expose only this container's port externally |
| Deployment complexity | Medium — one additional container in Docker Compose |
| Independent scalability | Yes — can be scaled or replicated independently |
| Security surface | Low — read-only, no admin endpoints |
| Failure isolation | Strong — MCP outage does not affect authoring |

**Pros:**
- Network security model is clean: companies can expose port 3002 (MCP) externally while keeping port 3000 (app) and 3001 (API) on an internal network only — a standard enterprise firewall pattern
- The MCP server has zero write access to the database or vector store, limiting the blast radius of a compromise
- Independent deployment allows the MCP server to be updated, restarted, or scaled without affecting the authoring experience
- Concerns are clearly separated: the MCP service owns retrieval logic; the API service owns content management logic
- Future scaling: if MCP query volume grows, the MCP service can be scaled horizontally without scaling the entire API

**Cons:**
- One additional Docker container to operate, document, and monitor
- Code sharing between the API and MCP server (e.g., VexFS client, PostgreSQL client) requires a shared internal library or package
- Developers working locally need to run the full Docker Compose stack to test end-to-end

---

### Option B: MCP Endpoint Embedded in Main API

| Dimension | Assessment |
|-----------|------------|
| Network isolation | None — exposing MCP means exposing the full API |
| Deployment complexity | Low — one fewer container |
| Independent scalability | No — MCP and API scale together |
| Security surface | Higher — same process handles both admin and public queries |
| Failure isolation | Poor — a MCP query spike affects authoring API performance |

**Pros:**
- Simpler Docker Compose configuration
- No code-sharing problem — all code lives in one service
- One fewer process to monitor

**Cons:**
- Cannot expose MCP publicly without also exposing admin API endpoints — requires complex API gateway or routing rules to compensate
- A malformed or excessive MCP query load can degrade authoring API performance (shared event loop in Node.js)
- Security audit surface is larger — the same process that handles user authentication and page writes also handles public queries
- Violates the single-responsibility principle at the service level

---

### Option C: Serverless MCP Function (AWS Lambda / Cloudflare Workers)

| Dimension | Assessment |
|-----------|------------|
| Network isolation | Strong |
| Deployment complexity | High — requires cloud provider, conflicts with self-hosted ethos |
| Independent scalability | Excellent — infinite scale |
| Security surface | Low |
| Failure isolation | Strong |

**Pros:**
- Zero ops for scaling
- Pay-per-query cost model

**Cons:**
- Fundamentally incompatible with the self-hosted, open-source model — requires a cloud provider
- Cold start latency conflicts with the < 2 second p95 requirement
- Rejected immediately — not viable for Historiador Doc's deployment model

---

## Trade-off Analysis

The choice between Option A and Option B comes down to one question: **who controls the network boundary?**

In Option B, the company's IT admin must configure an API gateway or reverse proxy with path-based routing rules to expose only `/mcp/*` routes publicly. This is doable but adds operational complexity outside of Historiador Doc's own Docker Compose configuration — it becomes the admin's responsibility, not the product's.

In Option A, the company simply exposes port 3002 and keeps ports 3000 and 3001 internal. This is a single firewall rule and requires no application-level routing configuration. The operational burden is lower and the security model is more auditable.

The additional Docker container is a modest cost for a meaningful security and isolation benefit. The code-sharing concern is manageable via a shared internal `@historiador/db-client` package or by duplicating the lightweight client code.

---

## Service Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Internal Network                          │
│                                                             │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────┐  │
│  │   Next.js    │    │  API Server  │    │  PostgreSQL  │  │
│  │  (port 3000) │───▶│  (port 3001) │───▶│  (port 5432) │  │
│  └──────────────┘    └──────┬───────┘    └──────────────┘  │
│                             │                               │
│                             ▼                               │
│                      ┌──────────────┐                       │
│                      │    VexFS     │                       │
│                      │  (port 8000) │                       │
│                      └──────┬───────┘                       │
│                             │  read-only                    │
└─────────────────────────────┼───────────────────────────────┘
                              │
┌─────────────────────────────┼───────────────────────────────┐
│            DMZ / Public-Facing                               │
│                             │                               │
│                      ┌──────▼───────┐                       │
│                      │  MCP Server  │◀── AI tools           │
│                      │  (port 3002) │    (Claude, Cursor...) │
│                      └──────────────┘                       │
└─────────────────────────────────────────────────────────────┘
```

**Write access summary:**
- Next.js → API: reads and writes (via API)
- API → PostgreSQL: reads and writes
- API → VexFS: reads and writes (chunker pipeline)
- MCP Server → VexFS: **read-only**
- MCP Server → PostgreSQL: **read-only** (page metadata for source attribution)

---

## Consequences

**Easier:**
- IT admins can expose only the MCP service port without complex routing configuration
- Security reviews can audit the MCP server in isolation — it has no write path
- MCP performance issues can be investigated and resolved without touching the authoring pipeline
- Future: MCP server can be replicated behind a load balancer for high-traffic workspaces without scaling the API

**Harder:**
- Local development requires running the full Docker Compose stack for end-to-end testing
- Shared database clients (VexFS, PostgreSQL) must be maintained consistently across both services — use a shared `packages/db` workspace in a monorepo structure
- Docker Compose documentation must clearly explain the two-port exposure model to IT admins

**Must revisit:**
- If v2 introduces streaming MCP responses, the standalone service architecture makes this straightforward to add without affecting the API
- If MCP query volume grows significantly, horizontal scaling of the MCP container should be documented as the first scaling lever

---

## Action Items

1. [ ] Define the Docker Compose service for the MCP server with read-only environment variable configuration (no write credentials)
2. [ ] Create a `packages/db` shared workspace in the monorepo for VexFS and PostgreSQL clients, shared between the API and MCP services
3. [ ] Document the two-port network exposure model in the installation guide with example firewall/nginx configuration
4. [ ] Implement bearer token validation as middleware in the MCP server
5. [ ] Write integration test that verifies MCP server cannot perform write operations against VexFS or PostgreSQL
