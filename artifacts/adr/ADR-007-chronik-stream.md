# ADR-007: Unified Event + Search Platform — Chronik-Stream

**Status:** Accepted
**Date:** 2026-04-12
**Supersedes:** [ADR-001](ADR-001-vector-database.md) (VexFS as vector store)
**Deciders:** Gabriel Specian (Nexian Tech)

---

## Context

ADR-001 selected VexFS as the vector store for chunk embeddings. This decision is superseded by the choice to use **Chronik-Stream** ([github.com/lspecian/chronik-stream](https://github.com/lspecian/chronik-stream)) as the unified event, search, and analytics platform for Historiador Doc.

Chronik-Stream is a high-performance distributed system written in Rust that consolidates multiple data infrastructure components into a single binary:

- **Kafka-compatible event streaming** (port 9092) — durable, ordered event log
- **Vector semantic search** — HNSW index for similarity queries
- **Full-text search** — Tantivy BM25 implementation (NDCG@10 of 0.5927 on WANDS, outperforming Elasticsearch by 6–18%)
- **SQL analytics** — DataFusion against Arrow/Parquet columnar storage (port 6092 REST API)
- **Per-topic capability model** — topics selectively enable indexing; a basic topic is pure streaming with zero overhead

All indexing is asynchronous post-durability. The write path is unaffected while background indexers populate Tantivy, Parquet files, and HNSW vectors. Content becomes queryable within 30–90 seconds of ingestion.

Like VexFS, Chronik-Stream is authored by the Specian family — giving Historiador Doc first-party access to the authors for integration, performance tuning, and feature development.

---

## Decision

**Replace VexFS with Chronik-Stream** as the core data platform for Historiador Doc. Chronik-Stream handles vector similarity search (replacing VexFS), full-text search (replacing PostgreSQL `tsvector`), event streaming (new capability), and query analytics (new capability). PostgreSQL is retained for relational data only.

---

## Why This Is More Than a Vector Store Swap

With VexFS, the architecture was:
```
PostgreSQL  → relational data + full-text search
VexFS       → vector embeddings
```

With Chronik-Stream:
```
PostgreSQL      → relational data (users, collections, page metadata, auth)
Chronik-Stream  → everything else: vectors, full-text, streaming, analytics
```

This eliminates PostgreSQL `tsvector` for page search, introduces a durable event log across the entire platform, and makes gap detection (v2) a natural stream-processing consequence rather than a batch analytics feature built on top.

---

## Topic Architecture

Each Historiador Doc capability maps to a Chronik topic with appropriate capabilities enabled:

| Topic | Capabilities | Purpose |
|-------|-------------|---------|
| `published-pages` | Vector index + Full-text | MCP semantic search; dashboard full-text search |
| `mcp-queries` | SQL analytics | Gap detection; usage reporting |
| `editor-conversations` | Streaming only | Durable conversation history for the split-pane editor |
| `page-events` | Streaming + SQL | Audit log; webhook notifications (v2) |

The per-topic model means the `editor-conversations` topic carries zero indexing overhead — it's a pure event stream. The `published-pages` topic carries both vector and full-text indexing because that's what the MCP server queries.

---

## Implications for Gap Detection (v2)

With VexFS, gap detection required a separate analytics layer to be built in v2. With Chronik-Stream, the infrastructure is already there:

1. Every MCP query is written to the `mcp-queries` topic on arrival
2. DataFusion SQL runs directly against the `mcp-queries` Parquet storage to find low-relevance-score clusters
3. A background job queries: *"Which question patterns in the last 7 days returned chunks with score < 0.6, grouped by semantic topic?"*
4. Results surface as documentation gap notifications

The gap detection flywheel no longer requires a separate data pipeline — it's a SQL query against an event topic that already exists. This moves gap detection from a complex v2 build to a relatively simple v2 query.

---

## Implications for the Conversational Editor

The `editor-conversations` topic stores every message exchange in the split-pane editor as a durable event. This means:

- Conversations that produce published pages are archived — the "how this was created" is preserved
- Conversations that were abandoned without publishing are visible — this is a different kind of documentation gap (the author tried but couldn't complete the doc)
- Future versions can resume an in-progress conversation across sessions
- The editorial history of a document is queryable

---

## Options Considered

### Option A: Chronik-Stream ✅ Selected

| Dimension | Assessment |
|-----------|------------|
| Vector search | HNSW — strong performance |
| Full-text search | Tantivy — outperforms Elasticsearch BM25 |
| Event streaming | Kafka-compatible — first-class |
| SQL analytics | DataFusion — production-ready |
| Team familiarity | High — first-party author access |
| Operational complexity | Low — single binary in Docker Compose |
| Rust stack alignment | Native — same language as all backend services |

**Pros:**
- Single binary replaces VexFS + PostgreSQL full-text + a future analytics service
- First-party author relationship (same as VexFS) — features and fixes are accessible directly
- Event streaming makes the MCP query log, editor conversation history, and gap detection a natural consequence of the architecture rather than features to build
- Kafka-compatible — standard consumer/producer libraries work with Chronik out of the box
- Validated at scale: 43.9M records, sub-200ms query latency at 1000 virtual users

**Cons:**
- Larger footprint than VexFS — more capability means more surface area to configure correctly
- Per-topic capability configuration requires careful planning upfront (see Topic Architecture above)
- The Raft clustering model (3-node minimum for production HA) adds operational complexity for large deployments — acceptable to run single-node in v1

### Option B: VexFS (ADR-001, superseded)

Purpose-built vector store. Good fit for v1 scope but requires PostgreSQL `tsvector` for full-text search and a separate analytics system for gap detection. Superseded because Chronik-Stream is a strict superset for this use case with the same first-party author relationship.

### Option C: Qdrant + Kafka (separate systems)

Strong vector search (Qdrant) + industry-standard streaming (Kafka). Better community documentation than Chronik-Stream.

**Rejected** — two third-party systems instead of one first-party system. No author relationship for either. Operational complexity of maintaining two separate services with no unifying query model.

---

## Consequences

**Easier:**
- Dashboard full-text page search uses Tantivy via Chronik — no `tsvector` indexing in PostgreSQL
- MCP semantic search uses HNSW via Chronik — same as before with VexFS, but now colocated with other indices
- Gap detection (v2) is a DataFusion SQL query against an existing topic — not a new infrastructure build
- Editor conversation history is durable by default — no separate storage needed

**Harder:**
- Topic configuration must be designed carefully upfront — enabling the wrong capabilities on a high-volume topic wastes resources
- Single-node v1 deployment is fine; production HA requires 3-node Raft cluster — document this clearly in the installation guide
- Chronik's 30–90 second indexing latency means newly published pages may not be queryable via MCP immediately — document this as expected behavior

**Must revisit:**
- Validate that Chronik-Stream has or will have a first-class Rust client library — same prerequisite as VexFS was in ADR-001
- Confirm HNSW embedding dimension configuration matches the LLM embedding model used (e.g., `text-embedding-3-small` = 1536 dims)
- Benchmark MCP query latency with Chronik HNSW against the p95 < 2s target before beta

---

## Action Items

1. [ ] Confirm Chronik-Stream Rust client library status with the Specian team
2. [ ] Define the four topic configurations in `docker-compose.yml` with correct capability flags
3. [ ] Update `crates/db` to replace the VexFS client interface with a Chronik client implementing the same `VectorStore` + `SearchStore` + `EventStore` traits
4. [ ] Validate HNSW embedding dimension configuration
5. [ ] Benchmark p50/p95 MCP query latency against a 10,000-chunk dataset in Chronik
6. [ ] Document the 30–90 second indexing delay as expected behavior in the installation guide
7. [ ] Update Docker Compose to replace the `vexfs` service with `chronik`
