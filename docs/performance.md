# Performance

MCP retrieval latency is the externally-visible v1.0 contract — if Claude
Desktop takes too long to get an answer, the product feels broken even
if the rest of the system is healthy. The v1.0 target is:

> **p95 < 2 s for 1,000 queries against 10,000 seeded chunks**, measured
> end-to-end against `POST /mcp` over loopback.

This document is the load-test record. It is updated on every release
cut and on any significant retrieval-path change (new vector store, new
HNSW parameters, embedding dimension change).

---

## How to run the load test

Prerequisites on the host running the load test:

- The full stack is up: `docker compose up -d` (Postgres + Ollama +
  Chronik) and the `api` + `mcp` binaries are running (in production
  mode: `docker compose -f docker-compose.prod.yml up -d`).
- `oha` and `jq` installed:
  ```bash
  cargo install oha
  # apt:   sudo apt install jq
  # brew:  brew install jq
  ```
- An MCP bearer token exported as `MCP_BEARER_TOKEN` (copy from the
  admin dashboard or the `.env` file).

Run:

```bash
# 1. Seed 10,000 synthetic chunks into Chronik.
cargo run --release -p historiador_api --bin load-test-seed -- \
    --chunks 10000 --dim 1536

# 2. Fire the load test.
export MCP_BEARER_TOKEN="<token>"
./scripts/load-test/run.sh --emit-report
```

`run.sh` executes 100 requests per phrase across 10 distinct phrases
(1,000 requests total) at concurrency 10, records p50/p95/p99 per
phrase, and appends an averaged summary to this document. The script
exits non-zero if the averaged p95 exceeds 2 s so CI can catch a
regression.

Tweak knobs via env vars if you need to explore:

- `MCP_URL` — default `http://localhost:3002/mcp`
- `TOTAL_PER_PHRASE` — default 100
- `CONCURRENCY` — default 10

---

## Latest result

> _Placeholder: the v1.0 release hardening sprint scaffolded the
> harness but did not execute the run in-session. The first real
> entry will be written by the script when it is invoked with
> `--emit-report`. Until then, treat the target below as an
> **aspiration**, not a measured number._

**Target:** p50 < 500 ms, p95 < 2 s, p99 < 4 s.

---

## Tuning levers

In priority order if p95 comes in above target:

1. **Chronik HNSW parameters** (`ef_search`, `m`) via Chronik server
   env vars. The `published-pages` topic is the hot one; it is
   declared in both `docker-compose.yml` and `docker-compose.prod.yml`
   with `vector,fulltext` capabilities.
2. **Axum connection pool** size in `apps/mcp`. The binary uses the
   sqlx default pool which is probably fine at v1.0 load but may
   contend with Chronik-heavy traffic.
3. **Query embedding cache** (60 s TTL, keyed by exact query string)
   inside the `SearchChunksUseCase`. Skips the embedding call for
   repeated queries — a common pattern in agent workflows that query
   the same topic multiple times during one task. Not implemented in
   v1.0; scoped as a v1.1 issue if the data says it matters.
4. **`cargo flamegraph`** on the MCP binary to find the real hotspot
   before further tuning. Run under load and sample for 30 s.

Document every tuning decision here with a new `## Run on YYYY-MM-DD`
section. The scripted runner appends automatically when passed
`--emit-report`.

---

## Known out-of-scope

- **No remote / cross-region measurements.** The v1.0 deploy model is
  single-host behind a reverse proxy. Add measurements if a customer
  deploys across regions.
- **No sustained soak.** The harness fires 1,000 requests then stops.
  Longer-running soak tests are a v1.1 concern once real-user traffic
  informs what we should measure.
- **Embedding provider latency is not measured.** The seed is done
  with the deterministic xorshift-RNG embeddings baked into the
  `load-test-seed` binary, not via a real provider. Query-path
  embeddings still go through the configured provider, so the
  recorded p95 includes the embedding call; a future change to use a
  stub embedder for the MCP load test would tighten the number.
