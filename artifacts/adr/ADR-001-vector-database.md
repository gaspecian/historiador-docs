# ADR-001: Vector Database Selection — VexFS

**Status:** Superseded by [ADR-007](ADR-007-chronik-stream.md)
**Date:** 2026-04-08
**Superseded:** 2026-04-12
**Deciders:** Gabriel Specian (Nexian Tech), VexFS core team

> ⚠️ This ADR has been superseded. The vector store was changed from VexFS to Chronik-Stream, which provides vector search, full-text search, streaming, and SQL analytics in a single Rust binary. See ADR-007 for the current decision.

---

## Context

Historiador Doc requires a vector database to store and query chunk embeddings. Every time a documentation page is published, its structure-aware chunks are embedded and written to the vector store. Every MCP query triggers a semantic similarity search against that store. The vector database is therefore on the critical path of both the write pipeline (publish → chunk → embed → store) and the read path (query → retrieve → respond).

Requirements for the vector store:
- Must run self-hosted with no external cloud dependency
- Must be embeddable in a Docker Compose setup alongside the rest of the stack
- Must support metadata filtering (language, collection, page ID) on similarity searches
- Must handle workloads typical for a company knowledge base (thousands to low tens-of-thousands of chunks per workspace)
- Must be maintainable by the Historiador Doc team with direct access to the database authors

---

## Decision

**Use VexFS** ([github.com/lspecian/vexfs](https://github.com/lspecian/vexfs)) as the vector database for all chunk embeddings.

VexFS is a purpose-built vector database developed by the Specian family. Historiador Doc has direct access to the core authors, making it a first-party dependency in practice. VexFS will be bundled in the Docker Compose deployment and treated as an internal component of the Historiador Doc stack rather than a third-party service.

---

## Options Considered

### Option A: VexFS ✅ Selected

| Dimension | Assessment |
|-----------|------------|
| Operational complexity | Low — bundled in Docker Compose, no external service |
| Self-hosted support | Native — designed for self-hosted deployment |
| Metadata filtering | Supported |
| Team familiarity | High — direct access to authors |
| Maintenance overhead | Low — can file issues or PRs directly |
| Licensing | Open source |

**Pros:**
- Direct relationship with authors means bugs and missing features can be fixed rather than worked around
- Fully aligned with Historiador Doc's open-source, self-hosted ethos
- No external cloud dependency — all embeddings stay inside the company's infrastructure
- Stack ownership: Historiador Doc controls both the documentation layer and the retrieval layer end-to-end

**Cons:**
- Smaller community and ecosystem than established alternatives
- Less documentation and third-party tooling compared to Qdrant or Weaviate
- Production maturity is less publicly proven at scale

---

### Option B: pgvector (PostgreSQL extension)

| Dimension | Assessment |
|-----------|------------|
| Operational complexity | Low — reuses existing PostgreSQL instance |
| Self-hosted support | Native |
| Metadata filtering | Supported via standard SQL |
| Team familiarity | Medium — SQL is universal, but vector-specific tuning requires expertise |
| Maintenance overhead | Low for basic use; higher at scale |
| Licensing | Open source |

**Pros:**
- Eliminates a separate service — one PostgreSQL instance handles both relational and vector data
- Mature, well-documented, large community
- SQL-native metadata filtering is flexible and familiar

**Cons:**
- Vector search performance degrades significantly at scale without careful indexing (IVFFlat or HNSW index tuning required)
- Mixing relational and vector workloads in one database increases operational coupling — a slow vector query can affect relational throughput
- Does not align with the first-party VexFS story

---

### Option C: Qdrant

| Dimension | Assessment |
|-----------|------------|
| Operational complexity | Medium — separate service, Docker-ready but requires configuration |
| Self-hosted support | Strong — designed for self-hosted |
| Metadata filtering | Strong — first-class payload filtering |
| Team familiarity | Low initially |
| Maintenance overhead | Medium — third-party dependency |
| Licensing | Open source (Apache 2.0) |

**Pros:**
- Purpose-built for vector search; strong performance at scale
- Excellent metadata filtering with typed payloads
- Well-documented REST and gRPC APIs
- Large and active community

**Cons:**
- Third-party dependency — feature gaps or bugs require upstream resolution
- Larger operational footprint than pgvector
- No strategic alignment with the Historiador Doc + VexFS narrative

---

### Option D: Weaviate

| Dimension | Assessment |
|-----------|------------|
| Operational complexity | High — resource-intensive, complex configuration |
| Self-hosted support | Available but heavy |
| Metadata filtering | Strong |
| Team familiarity | Low |
| Maintenance overhead | High |
| Licensing | Open source (BSD) |

**Pros:**
- Rich feature set including built-in module ecosystem
- Strong multi-tenancy support (useful for v2 multi-workspace)

**Cons:**
- Significantly heavier than other options — high memory footprint conflicts with the "2 vCPU / 4GB" minimum install target
- Overkill for v1 workloads
- Steep learning curve

---

## Trade-off Analysis

The core trade-off is **ecosystem maturity vs. strategic alignment**.

pgvector and Qdrant are the more battle-tested choices. If Historiador Doc were a standalone product with no relationship to VexFS, Qdrant would be the strongest recommendation for a purpose-built vector store at self-hosted scale.

However, Historiador Doc is being built by the same family that built VexFS. This creates a durable competitive advantage: when the retrieval layer needs a capability (e.g., language-filtered similarity search, chunk-level scoring tuning), the Historiador Doc team can build it into VexFS directly rather than waiting on a third-party roadmap. This tight feedback loop between the application and its data layer is rare and valuable in open-source development.

The risk — lower public maturity — is mitigated by the team's direct access to the codebase.

---

## Consequences

**Easier:**
- Retrieval behavior can be tuned precisely for documentation workloads, with direct collaboration between the Historiador Doc and VexFS teams
- Feature requests (e.g., language metadata filtering, chunk scoring) can be implemented in VexFS without depending on a third-party release cycle
- The combined Historiador Doc + VexFS narrative is a compelling open-source story

**Harder:**
- External contributors to Historiador Doc need to familiarize themselves with VexFS (less familiar than Qdrant or pgvector)
- VexFS documentation gaps may need to be addressed as part of the Historiador Doc onboarding experience
- VexFS must be validated at the document-scale workloads Historiador Doc expects; load testing is required before beta

**Must revisit:**
- If VexFS cannot meet the < 2 second p95 MCP response target under realistic load, the team must either optimize VexFS or evaluate a migration path to Qdrant — the API layer between the MCP server and the vector store should be abstracted to make this possible

---

## Action Items

1. [ ] Define the VexFS client interface used by the Historiador Doc API — abstract it behind a `VectorStore` interface so the underlying implementation can be swapped without changing MCP server or chunker code
2. [ ] Validate VexFS Docker image size and startup time against the 2 vCPU / 4GB install target
3. [ ] Confirm VexFS supports metadata filtering on `language`, `page_id`, `collection_path`, and `last_updated` fields
4. [ ] Run load test: simulate 10,000 chunks in VexFS and measure p50/p95 query latency
5. [ ] Document VexFS configuration options relevant to Historiador Doc in the installation guide
