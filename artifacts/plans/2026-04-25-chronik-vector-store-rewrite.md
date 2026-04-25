# Chronik Vector Store Rewrite — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make page publish actually index chunks in Chronik so the MCP server can return them, by replacing the fictional `/api/v1/*` REST calls with the real Chronik 2.4.1 API surface (Kafka produce on :9092 for writes, `/_vector/.../search` and `/_sql` for reads).

**Architecture:** Chronik 2.4.1 has no HTTP write path — writes must go through the Kafka wire protocol on port 9092 against a topic configured `vector.enabled=true`. Chronik embeds records asynchronously via its built-in pipeline (configured to use OpenAI). Vector hits return `(partition, offset, score, text_preview)`, not custom IDs, so we store the `(partition, offset)` pair in the `chunks` Postgres row and join on it during MCP enrichment. On republish we delete the Postgres rows and re-produce all chunks (full reindex); orphaned Chronik records get filtered at enrichment time. Page produce stays fire-and-forget (existing behavior).

**Tech Stack:** Rust (workspace), `rskafka` (pure-Rust async Kafka client), `reqwest` (HTTP), `sqlx` (Postgres), `axum` (API/MCP). Chronik-Stream 2.4.1 on Kafka :9092 + REST :6092.

**Decisions (recorded so executors don't re-derive them):**
1. **Chronik owns embedding.** The `published-pages` topic is created with `vector.embedding.provider=openai`. Our `EmbeddingClient` is removed from the chunk pipeline and from MCP search.
2. **Hit→chunk mapping** lives in `chunks.chronik_partition` + `chunks.chronik_offset` (new columns). Old `vexfs_ref` column is dropped.
3. **Republish = full reindex.** Pipeline deletes Postgres `chunks` rows for the page version, then produces all chunks again. Stale Chronik records become orphans (no Postgres row) and are filtered during MCP enrichment.
4. **Fire-and-forget stays.** Publish keeps `tokio::spawn`-ing the pipeline. Errors log via `tracing::warn!` (already in place).
5. **Pure-Rust Kafka client: `rskafka`.** Async, used by InfluxDB IOx. No `librdkafka` C dep.

---

## File Structure

**New files:**
- `crates/db/src/chronik/kafka_producer.rs` — thin async wrapper around `rskafka` for produce + topic admin
- `crates/db/migrations/0008_chunks_chronik_offsets.sql` — drop `vexfs_ref`, add `chronik_partition`, `chronik_offset`
- `apps/mcp/tests/mcp_query_e2e.rs` — end-to-end integration test (publish page → wait → MCP query → assert)

**Modified files:**
- `crates/db/Cargo.toml` — add `rskafka`
- `crates/db/src/chronik/mod.rs` — register module, hold `Arc<KafkaProducer>` on `ChronikClient`, update docstring
- `crates/db/src/chronik/producer.rs` — rewrite `produce_event` to use Kafka
- `crates/db/src/chronik/search.rs` — delete `upsert_chunks` and `delete_by_page_version`; rewrite `vector_search` to call `POST /_vector/published-pages/search` with text query
- `crates/db/src/chronik/analytics.rs` — `/api/v1/sql` → `/_sql`
- `crates/db/src/vector_store.rs` — drop `HttpVexfsClient` (dead VexFS stub); change `VectorStore::search` to take `query: &str` (not embedding); change `ChunkRef` to carry `(partition, offset)`; change `upsert_chunks` to `produce_chunks` returning `Vec<(i32, i64)>`; drop `delete_by_page_version`; rewrite `ChronikVectorStore` accordingly; rewrite `InMemoryVectorStore` to do substring scoring (still useful for unit tests of the use case layer)
- `crates/db/src/postgres/chunks.rs` — `NewChunk` and `ChunkRow` lose `vexfs_ref`, gain `chronik_partition: i32` + `chronik_offset: i64`; insert and select queries updated
- `crates/db/src/postgres/mcp_queries.rs` — `enrich_chunk_results` keyed by `(chronik_partition, chronik_offset)` returning `HashMap<(i32, i64), ChunkMetadata>` plus a content/heading_path map (since we no longer trust Chronik’s `text_preview` for full content)
- `apps/api/src/infrastructure/chunker/pipeline.rs` — drop embedding step, drop `EmbeddingClient` field, do full-reindex (delete then produce), store `(partition, offset)`
- `apps/api/src/state.rs` and `apps/api/src/main.rs` — drop `embedding_client` from chunk-pipeline construction; pass `kafka_broker` to `ChronikClient`
- `apps/api/src/routes.rs` and use-case wiring — propagate the constructor signature change
- `apps/mcp/src/application/search_chunks.rs` — drop `EmbeddingClient` field; call `vector_store.search(&query, ...)` with text; enrich by `(partition, offset)` instead of `page_version_id`
- `apps/mcp/src/application/port.rs` and `apps/mcp/src/infrastructure/postgres_readonly.rs` — `ChunkMetadataReader::enrich_many` takes `&[(i32, i64)]` and returns `HashMap<(i32, i64), ChunkMetadata>` with `content` and `heading_path` included
- `apps/mcp/src/main.rs` — drop `EmbeddingClient` wiring; pass `kafka_broker` to `ChronikClient`
- `apps/api/examples/seed_chunks.rs` and `apps/api/src/bin/load_test_seed.rs` — update to new constructor + use Kafka produce path
- `docker-compose.yml` — pass `OPENAI_API_KEY` env to `chronik` service
- `.env.example` — document `CHRONIK_KAFKA_BROKER` + the OpenAI key requirement for Chronik
- `crates/db/src/chronik/mod.rs` docstring + topic comment table — update to reflect Kafka writes, REST reads only

---

## Tasks

### Task 1: Add Kafka offset columns to `chunks` table

**Files:**
- Create: `crates/db/migrations/0008_chunks_chronik_offsets.sql`

- [ ] **Step 1: Write the migration**

Create `crates/db/migrations/0008_chunks_chronik_offsets.sql`:

```sql
-- 0008: replace opaque vexfs_ref with the (partition, offset) pair
-- Chronik 2.4.1 returns from /_vector/<topic>/search hits.
--
-- chunks.vexfs_ref was a placeholder for the old "opaque pointer to a
-- VexFS doc id" model. Chronik does not assign user-controllable doc
-- ids; instead, every produced record is addressable by its
-- (partition, offset) within the topic. The MCP enrichment join now
-- keys on (chronik_partition, chronik_offset).

ALTER TABLE chunks DROP COLUMN vexfs_ref;

ALTER TABLE chunks
    ADD COLUMN chronik_partition INTEGER,
    ADD COLUMN chronik_offset    BIGINT;

-- Old chunk rows (if any) cannot be back-filled — they reference a
-- vector store that never received them. Keep the rows for audit but
-- mark the offsets as NULL; the MCP enricher must skip rows where
-- either column is NULL.

CREATE INDEX chunks_chronik_offset_idx
    ON chunks(chronik_partition, chronik_offset)
    WHERE chronik_partition IS NOT NULL;
```

- [ ] **Step 2: Run the migration locally to verify it applies**

Run:

```bash
sqlx migrate run --source crates/db/migrations \
  --database-url "postgres://historiador_admin:devpassword@localhost:5432/historiador"
```

Expected: `Applied 8/migrate chunks chronik offsets`

- [ ] **Step 3: Verify the new columns**

Run:

```bash
psql "postgres://historiador_admin:devpassword@localhost:5432/historiador" \
  -c "\d chunks" | grep -E "chronik_(partition|offset)|vexfs_ref"
```

Expected: two lines for `chronik_partition integer`, `chronik_offset bigint`. No `vexfs_ref` line.

- [ ] **Step 4: Commit**

```bash
git add crates/db/migrations/0008_chunks_chronik_offsets.sql
git commit -m "feat(db): replace chunks.vexfs_ref with chronik (partition, offset)"
```

---

### Task 2: Add `rskafka` dependency

**Files:**
- Modify: `crates/db/Cargo.toml`

- [ ] **Step 1: Add the dep**

In `crates/db/Cargo.toml`, under `[dependencies]`, add:

```toml
rskafka = { version = "0.5", default-features = false, features = ["compression-snappy"] }
```

- [ ] **Step 2: Verify it compiles**

Run:

```bash
cargo build -p historiador_db
```

Expected: build succeeds. (If `rskafka` 0.5 isn't on crates.io with these features, fall back to the latest 0.x version printed by `cargo search rskafka` and use that exact version.)

- [ ] **Step 3: Commit**

```bash
git add crates/db/Cargo.toml Cargo.lock
git commit -m "chore(db): add rskafka for Chronik Kafka-protocol produce"
```

---

### Task 3: Build the `KafkaProducer` wrapper

**Files:**
- Create: `crates/db/src/chronik/kafka_producer.rs`
- Modify: `crates/db/src/chronik/mod.rs`
- Test: `crates/db/tests/kafka_producer_chronik.rs`

- [ ] **Step 1: Write the failing integration test**

Create `crates/db/tests/kafka_producer_chronik.rs`:

```rust
//! Integration test against a running Chronik 2.4.1 on localhost:9092.
//! Skipped if `CHRONIK_KAFKA_BROKER` is not set.

use historiador_db::chronik::kafka_producer::{KafkaProducer, ProducedRecord};
use serde_json::json;

fn broker() -> Option<String> {
    std::env::var("CHRONIK_KAFKA_BROKER").ok()
}

#[tokio::test]
async fn produce_returns_partition_and_offset() {
    let Some(broker) = broker() else {
        eprintln!("skipping: CHRONIK_KAFKA_BROKER not set");
        return;
    };

    let producer = KafkaProducer::connect(&broker).await.expect("connect");

    // Use a throwaway topic with a unique name per run.
    let topic = format!("test-kp-{}", uuid::Uuid::new_v4());
    producer
        .ensure_topic(&topic, /*partitions*/ 1, None)
        .await
        .expect("ensure topic");

    let record = producer
        .produce(&topic, "key-1", &json!({"hello": "world"}))
        .await
        .expect("produce");

    assert_eq!(record.partition, 0);
    assert!(record.offset >= 0);
}
```

- [ ] **Step 2: Run the test to verify it fails (no module yet)**

Run:

```bash
cargo test -p historiador_db --test kafka_producer_chronik
```

Expected: compile error — `kafka_producer` module does not exist.

- [ ] **Step 3: Create the producer module**

Create `crates/db/src/chronik/kafka_producer.rs`:

```rust
//! Pure-Rust Kafka producer for Chronik-Stream writes.
//!
//! Chronik 2.4.1 has no HTTP write path. All ingest must go through
//! the Kafka wire protocol on port 9092. This wrapper exposes a
//! produce-and-await-offset call plus topic provisioning with vector
//! search config.

use std::collections::BTreeMap;
use std::sync::Arc;

use rskafka::client::partition::{Compression, UnknownTopicHandling};
use rskafka::client::{Client, ClientBuilder};
use rskafka::record::Record;
use serde_json::Value;
use thiserror::Error;
use time::OffsetDateTime;

#[derive(Debug, Error)]
pub enum KafkaProducerError {
    #[error("kafka client error: {0}")]
    Client(String),
    #[error("kafka produce error: {0}")]
    Produce(String),
    #[error("kafka admin error: {0}")]
    Admin(String),
    #[error("payload serialization failed: {0}")]
    Serialize(#[from] serde_json::Error),
}

/// The address of a produced record inside a Chronik topic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProducedRecord {
    pub partition: i32,
    pub offset: i64,
}

#[derive(Clone)]
pub struct KafkaProducer {
    client: Arc<Client>,
}

impl KafkaProducer {
    /// Connect to the Chronik Kafka endpoint. `broker` is `host:port`
    /// (e.g. `localhost:9092`).
    pub async fn connect(broker: &str) -> Result<Self, KafkaProducerError> {
        let client = ClientBuilder::new(vec![broker.to_string()])
            .build()
            .await
            .map_err(|e| KafkaProducerError::Client(e.to_string()))?;
        Ok(Self {
            client: Arc::new(client),
        })
    }

    /// Create a topic if it does not already exist. `topic_config` is
    /// merged into the create-topics request — used for
    /// `vector.enabled=true` and friends per the Chronik VECTOR_SEARCH_GUIDE.
    pub async fn ensure_topic(
        &self,
        topic: &str,
        partitions: i32,
        topic_config: Option<BTreeMap<String, String>>,
    ) -> Result<(), KafkaProducerError> {
        let controller = self
            .client
            .controller_client()
            .map_err(|e| KafkaProducerError::Admin(e.to_string()))?;

        // Check if the topic already exists.
        let topics = self
            .client
            .list_topics()
            .await
            .map_err(|e| KafkaProducerError::Admin(e.to_string()))?;
        if topics.iter().any(|t| t.name == topic) {
            return Ok(());
        }

        let cfg: Vec<(String, Option<String>)> = topic_config
            .unwrap_or_default()
            .into_iter()
            .map(|(k, v)| (k, Some(v)))
            .collect();

        controller
            .create_topic(
                topic,
                partitions,
                /*replication_factor*/ 1,
                /*timeout_ms*/ 5_000,
            )
            .await
            .map_err(|e| KafkaProducerError::Admin(e.to_string()))?;

        // rskafka's create_topic does not accept config. Apply the
        // per-topic config via the AlterConfigs admin call.
        if !cfg.is_empty() {
            controller
                .alter_resource_configs(
                    rskafka::client::controller::ConfigResource::Topic(topic.to_string()),
                    cfg,
                )
                .await
                .map_err(|e| KafkaProducerError::Admin(e.to_string()))?;
        }

        Ok(())
    }

    /// Produce a single JSON record. Returns the assigned
    /// `(partition, offset)` so the caller can persist a back-pointer.
    pub async fn produce(
        &self,
        topic: &str,
        key: &str,
        payload: &Value,
    ) -> Result<ProducedRecord, KafkaProducerError> {
        let value_bytes = serde_json::to_vec(payload)?;

        // Single-partition produce. rskafka picks partition 0 by
        // default; if the topic has more partitions we hash on key.
        let topics = self
            .client
            .list_topics()
            .await
            .map_err(|e| KafkaProducerError::Client(e.to_string()))?;
        let partition_count = topics
            .iter()
            .find(|t| t.name == topic)
            .map(|t| t.partitions.len() as i32)
            .unwrap_or(1);
        let partition = if partition_count <= 1 {
            0
        } else {
            (xxhash_rust::xxh3::xxh3_64(key.as_bytes()) % partition_count as u64) as i32
        };

        let partition_client = self
            .client
            .partition_client(topic, partition, UnknownTopicHandling::Error)
            .await
            .map_err(|e| KafkaProducerError::Client(e.to_string()))?;

        let record = Record {
            key: Some(key.as_bytes().to_vec()),
            value: Some(value_bytes),
            headers: BTreeMap::new(),
            timestamp: OffsetDateTime::now_utc(),
        };

        let offsets = partition_client
            .produce(vec![record], Compression::Snappy)
            .await
            .map_err(|e| KafkaProducerError::Produce(e.to_string()))?;

        let offset = *offsets.first().ok_or_else(|| {
            KafkaProducerError::Produce("kafka returned no offset".to_string())
        })?;
        Ok(ProducedRecord { partition, offset })
    }
}
```

- [ ] **Step 4: Wire the new module**

In `crates/db/src/chronik/mod.rs`, under `pub mod analytics;` etc., add:

```rust
pub mod kafka_producer;
```

Also add this dep to `crates/db/Cargo.toml` (used for partition hashing):

```toml
xxhash-rust = { version = "0.8", features = ["xxh3"] }
time = { version = "0.3", default-features = false }
```

- [ ] **Step 5: Run the failing integration test, set the env var**

Run:

```bash
CHRONIK_KAFKA_BROKER=localhost:9092 cargo test -p historiador_db --test kafka_producer_chronik -- --nocapture
```

Expected: PASS. The test creates a unique topic, produces one record, asserts an offset is returned.

If `rskafka` 0.5 doesn't have `alter_resource_configs` with this exact signature, replace that block with the closest equivalent from the version actually published. The unit-tested behavior here is `produce` returning a valid offset; topic config is exercised in Task 4 against the real `published-pages` topic.

- [ ] **Step 6: Commit**

```bash
git add crates/db/src/chronik/kafka_producer.rs crates/db/src/chronik/mod.rs \
        crates/db/Cargo.toml Cargo.lock crates/db/tests/kafka_producer_chronik.rs
git commit -m "feat(db): KafkaProducer wrapper for Chronik writes"
```

---

### Task 4: Provision the `published-pages` topic with vector indexing

**Files:**
- Modify: `crates/db/src/chronik/kafka_producer.rs` (add topic spec helper)
- Modify: `apps/api/src/main.rs` (call ensure on boot)

- [ ] **Step 1: Add the topic spec helper**

In `crates/db/src/chronik/kafka_producer.rs`, append:

```rust
/// Topic configuration matching the chunk pipeline expectations.
/// Mirrors VECTOR_SEARCH_GUIDE.md.
pub fn published_pages_topic_config() -> BTreeMap<String, String> {
    let mut cfg = BTreeMap::new();
    cfg.insert("vector.enabled".into(), "true".into());
    cfg.insert("vector.embedding.provider".into(), "openai".into());
    cfg.insert("vector.embedding.model".into(), "text-embedding-3-small".into());
    cfg.insert("vector.field".into(), "$.content".into());
    cfg.insert("vector.index.type".into(), "hnsw".into());
    cfg.insert("vector.index.metric".into(), "cosine".into());
    cfg
}
```

- [ ] **Step 2: Provision on API boot**

In `apps/api/src/main.rs`, after the `ChronikClient` is constructed (around the existing `match ChronikClient::new(...)` block near line 118), add:

```rust
// Ensure the published-pages topic exists with vector indexing.
// Idempotent: skipped if the topic is already present.
let kafka = chronik.kafka_producer.clone();
if let Err(e) = kafka
    .ensure_topic(
        historiador_db::chronik::producer::topics::PUBLISHED_PAGES,
        /*partitions*/ 6,
        Some(historiador_db::chronik::kafka_producer::published_pages_topic_config()),
    )
    .await
{
    tracing::error!(error = %e, "failed to ensure published-pages topic — chunk pipeline writes will fail");
}

// Streaming-only topics (no vector indexing).
for topic in [
    historiador_db::chronik::producer::topics::PAGE_EVENTS,
    historiador_db::chronik::producer::topics::MCP_QUERIES,
    historiador_db::chronik::producer::topics::EDITOR_CONVERSATIONS,
] {
    if let Err(e) = kafka.ensure_topic(topic, /*partitions*/ 3, None).await {
        tracing::warn!(%topic, error = %e, "failed to ensure topic");
    }
}
```

(`chronik.kafka_producer` is added in Task 5 — this code references the field that will exist after that task. Order: do Task 5 before re-running, or simply add the wiring scaffold in this task as commented-out and fill after Task 5. Either way, this commit produces a build error until Task 5 lands. Don't commit Task 4 until Task 5 is also ready — or merge them. **Recommended:** stage Task 4 as part of Task 5's commit instead. Keep this section as documentation of intent and skip the standalone commit.)

- [ ] **Step 3: Defer the commit**

No commit yet. Hold the diff in the working tree until Task 5 is done; commit both together.

---

### Task 5: Wire `KafkaProducer` into `ChronikClient`

**Files:**
- Modify: `crates/db/src/chronik/mod.rs`
- Modify: `apps/api/src/main.rs`
- Modify: `apps/mcp/src/main.rs`
- Modify: `apps/api/examples/seed_chunks.rs`
- Modify: `apps/api/src/bin/load_test_seed.rs`
- Modify: `.env.example`

- [ ] **Step 1: Extend `ChronikClient` and `ChronikConfig`**

Replace the body of `crates/db/src/chronik/mod.rs` from the `pub struct ChronikClient` down with:

```rust
use std::sync::Arc;

use reqwest::Client;

use crate::chronik::kafka_producer::KafkaProducer;

/// Unified Chronik-Stream client. REST handles search + SQL; Kafka
/// handles writes (Chronik 2.4.1 has no HTTP write path).
#[derive(Clone)]
pub struct ChronikClient {
    /// REST base URL (search + SQL). E.g. `http://localhost:6092`.
    pub base_url: String,
    /// Search base URL — same host in single-port deploys, different
    /// host in split deploys.
    pub search_base_url: String,
    /// HTTP client for all REST API calls.
    pub http: Client,
    /// Kafka producer for writes (publish, page-events, etc.).
    pub kafka_producer: Arc<KafkaProducer>,
}

pub struct ChronikConfig {
    pub base_url: String,
    pub search_base_url: String,
    /// Kafka broker `host:port`.
    pub kafka_broker: String,
}

impl ChronikClient {
    pub async fn new(config: ChronikConfig) -> anyhow::Result<Self> {
        let kafka = KafkaProducer::connect(&config.kafka_broker)
            .await
            .map_err(|e| anyhow::anyhow!("connect to chronik kafka {}: {e}", config.kafka_broker))?;
        Ok(Self {
            base_url: config.base_url,
            search_base_url: config.search_base_url,
            http: Client::new(),
            kafka_producer: Arc::new(kafka),
        })
    }
}
```

- [ ] **Step 2: Update the docstring at the top of `mod.rs`**

Replace the file header (lines 1–18) with:

```rust
//! Chronik-Stream client — Kafka writes (port 9092), REST reads
//! (search + SQL on port 6092). ADR-007.
//!
//! Chronik 2.4.1 has no HTTP write endpoint. Producers must use the
//! Kafka wire protocol; the REST surface is read-only (search,
//! analytics SQL, health). See `kafka_producer` for the write path
//! and `search` / `analytics` for the read paths.
//!
//! # Topic Architecture
//!
//! | Topic                  | Capabilities       | Purpose                            |
//! |------------------------|--------------------|------------------------------------|
//! | `published-pages`      | Vector + Full-text | MCP semantic search; dashboard FTS |
//! | `mcp-queries`          | SQL analytics      | Gap detection; usage reporting     |
//! | `editor-conversations` | Streaming only     | Durable conversation history       |
//! | `page-events`          | Streaming + SQL    | Audit log; webhook notifications   |
```

- [ ] **Step 3: Update API and MCP main to pass the broker**

In `apps/api/src/main.rs`, find the existing `match ChronikClient::new(ChronikConfig {` (around line 118) and replace the construction site with:

```rust
let kafka_broker = std::env::var("CHRONIK_KAFKA_BROKER")
    .unwrap_or_else(|_| "localhost:9092".to_string());
let chronik = match ChronikClient::new(ChronikConfig {
    base_url: chronik_sql_url.clone(),
    search_base_url: chronik_search_url.clone(),
    kafka_broker,
})
.await
{
    Ok(c) => c,
    Err(e) => {
        tracing::error!(error = %e, "chronik client init failed");
        std::process::exit(1);
    }
};
```

(Keep whatever variable names already exist for the SQL/search URLs; only the `kafka_broker` field and `.await` are new.)

Apply the same change to `apps/mcp/src/main.rs` near line 131.

For `apps/api/examples/seed_chunks.rs:80` and `apps/api/src/bin/load_test_seed.rs:90`, add the same `kafka_broker` line and `.await` to their `ChronikClient::new` calls.

- [ ] **Step 4: Add Task 4 wiring**

Now add the Task 4 boot-time topic-ensure block to `apps/api/src/main.rs` immediately after the `ChronikClient` is constructed. (Code already drafted in Task 4 Step 2.)

- [ ] **Step 5: Add `CHRONIK_KAFKA_BROKER` to `.env.example`**

In `.env.example`, find the existing `CHRONIK_*` block (around line 67) and ensure it contains:

```
CHRONIK_KAFKA_BROKER=localhost:9092
CHRONIK_SQL_URL=http://localhost:6092
CHRONIK_SEARCH_URL=http://localhost:6092
```

(`CHRONIK_KAFKA_BROKER` is already implied via `HOST_PORT_CHRONIK_KAFKA=9092`; add the dedicated env var for clarity.)

- [ ] **Step 6: Verify the workspace compiles**

Run:

```bash
cargo build --workspace
```

Expected: build succeeds.

- [ ] **Step 7: Commit**

```bash
git add crates/db/src/chronik/mod.rs crates/db/src/chronik/kafka_producer.rs \
        apps/api/src/main.rs apps/mcp/src/main.rs \
        apps/api/examples/seed_chunks.rs apps/api/src/bin/load_test_seed.rs \
        .env.example
git commit -m "feat(chronik): wire Kafka producer through ChronikClient and provision topics on boot"
```

---

### Task 6: Rewrite `produce_event` to use Kafka

**Files:**
- Modify: `crates/db/src/chronik/producer.rs`
- Test: `crates/db/tests/event_producer_chronik.rs`

- [ ] **Step 1: Write the failing integration test**

Create `crates/db/tests/event_producer_chronik.rs`:

```rust
use historiador_db::chronik::{ChronikClient, ChronikConfig};
use serde_json::json;

#[tokio::test]
async fn produce_event_lands_in_chronik() {
    let Some(broker) = std::env::var("CHRONIK_KAFKA_BROKER").ok() else {
        eprintln!("skipping: CHRONIK_KAFKA_BROKER not set");
        return;
    };
    let client = ChronikClient::new(ChronikConfig {
        base_url: "http://localhost:6092".into(),
        search_base_url: "http://localhost:6092".into(),
        kafka_broker: broker,
    })
    .await
    .expect("client");

    // Use a throwaway topic.
    let topic = format!("test-events-{}", uuid::Uuid::new_v4());
    client
        .kafka_producer
        .ensure_topic(&topic, 1, None)
        .await
        .expect("ensure");

    client
        .produce_event(&topic, "k", &json!({"event": "ping"}))
        .await
        .expect("produce");
}
```

- [ ] **Step 2: Run the test — fails because `produce_event` still calls REST**

```bash
CHRONIK_KAFKA_BROKER=localhost:9092 cargo test -p historiador_db --test event_producer_chronik -- --nocapture
```

Expected: FAIL with `chronik produce error (404 Not Found)` — the bug we're fixing.

- [ ] **Step 3: Replace the REST body with Kafka produce**

In `crates/db/src/chronik/producer.rs`, replace the `impl ChronikClient` block with:

```rust
impl ChronikClient {
    /// Produce a JSON event to a Chronik topic via the Kafka wire
    /// protocol. `key` is used for partitioning.
    pub async fn produce_event(
        &self,
        topic: &str,
        key: &str,
        payload: &serde_json::Value,
    ) -> anyhow::Result<()> {
        self.kafka_producer
            .produce(topic, key, payload)
            .await
            .map_err(|e| anyhow::anyhow!("chronik produce ({topic}): {e}"))?;
        Ok(())
    }

    /// Fire-and-forget event production. Logs errors but never blocks
    /// the caller. Use for non-critical telemetry (MCP query logging,
    /// page events).
    pub fn produce_event_fire_and_forget(
        &self,
        topic: &'static str,
        key: String,
        payload: serde_json::Value,
    ) {
        let client = self.clone();
        tokio::spawn(async move {
            if let Err(e) = client.produce_event(topic, &key, &payload).await {
                tracing::warn!(
                    %topic,
                    error = %e,
                    "fire-and-forget event production failed"
                );
            }
        });
    }
}
```

- [ ] **Step 4: Re-run the test**

```bash
CHRONIK_KAFKA_BROKER=localhost:9092 cargo test -p historiador_db --test event_producer_chronik -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/db/src/chronik/producer.rs crates/db/tests/event_producer_chronik.rs
git commit -m "fix(chronik): produce events via Kafka, not the non-existent REST upsert"
```

---

### Task 7: Rewrite `VectorStore` trait + `ChronikVectorStore` for Kafka writes & real search endpoint

**Files:**
- Modify: `crates/db/src/vector_store.rs` (substantial)
- Modify: `crates/db/src/chronik/search.rs`

- [ ] **Step 1: Write the failing integration test for the new produce + search shape**

Create `crates/db/tests/chronik_vector_store_e2e.rs`:

```rust
use std::sync::Arc;

use historiador_db::chronik::kafka_producer::published_pages_topic_config;
use historiador_db::chronik::{ChronikClient, ChronikConfig};
use historiador_db::vector_store::{ChunkPayload, ChronikVectorStore, SearchFilters, VectorStore};

#[tokio::test]
async fn produce_then_search_recovers_partition_offset() {
    let Some(broker) = std::env::var("CHRONIK_KAFKA_BROKER").ok() else {
        eprintln!("skipping: CHRONIK_KAFKA_BROKER not set");
        return;
    };

    let client = ChronikClient::new(ChronikConfig {
        base_url: "http://localhost:6092".into(),
        search_base_url: "http://localhost:6092".into(),
        kafka_broker: broker,
    })
    .await
    .expect("client");

    // Ensure the topic with vector indexing.
    client
        .kafka_producer
        .ensure_topic(
            historiador_db::chronik::producer::topics::PUBLISHED_PAGES,
            6,
            Some(published_pages_topic_config()),
        )
        .await
        .expect("ensure");

    let store = ChronikVectorStore::new(client);
    let pv_id = uuid::Uuid::new_v4().to_string();

    let payloads = vec![ChunkPayload {
        page_version_id: pv_id.clone(),
        section_index: 0,
        heading_path: vec!["Intro".into()],
        content: "The quick brown fox jumps over the lazy dog.".into(),
        language: "en".into(),
        token_count: 9,
    }];

    let produced = store.produce_chunks(payloads).await.expect("produce");
    assert_eq!(produced.len(), 1);
    assert!(produced[0].offset >= 0);

    // Allow Chronik's background indexer to catch up. Polling rather
    // than fixed sleep — the test envelope still bounds runtime.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let mut hits = vec![];
    while std::time::Instant::now() < deadline {
        let results = store
            .search(
                "fox jumping over a dog",
                SearchFilters {
                    language: Some("en".into()),
                    page_version_id: Some(pv_id.clone()),
                },
                5,
            )
            .await
            .expect("search");
        if !results.is_empty() {
            hits = results;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }

    assert!(!hits.is_empty(), "expected at least one chunk back from Chronik within 60s");
    let first = &hits[0];
    assert_eq!(first.partition, produced[0].partition);
    assert_eq!(first.offset, produced[0].offset);
}
```

- [ ] **Step 2: Run the test — fails (types don't exist yet)**

```bash
CHRONIK_KAFKA_BROKER=localhost:9092 cargo test -p historiador_db --test chronik_vector_store_e2e -- --nocapture
```

Expected: compile error — `ChunkPayload`, `produce_chunks`, new `search` signature don't exist.

- [ ] **Step 3: Rewrite `vector_store.rs`**

Replace the entire body of `crates/db/src/vector_store.rs` with:

```rust
//! `VectorStore` trait + implementations.
//!
//! Production backend is Chronik-Stream (ADR-007). Writes go through
//! Kafka (Chronik has no HTTP write path); reads go through Chronik's
//! REST `/_vector/<topic>/search` (text query, Chronik embeds).
//! Hits are addressed by `(partition, offset)` because Chronik does
//! not assign user-controllable doc ids.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;
use thiserror::Error;

use crate::chronik::kafka_producer::ProducedRecord;

#[derive(Debug, Error)]
pub enum VectorStoreError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("kafka error: {0}")]
    Kafka(String),

    #[error("internal error: {0}")]
    Internal(String),
}

/// A chunk's payload as published to Chronik. Chronik embeds the
/// `content` field per the topic's `vector.field=$.content` config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkPayload {
    pub page_version_id: String,
    pub section_index: i32,
    pub heading_path: Vec<String>,
    pub content: String,
    pub language: String,
    pub token_count: i32,
}

/// A reference to a chunk located in Chronik, paired with the
/// similarity score from a search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkRef {
    pub partition: i32,
    pub offset: i64,
    pub score: f32,
    /// `text_preview` from Chronik (may be truncated). The
    /// authoritative content lives in the Postgres `chunks` row joined
    /// during MCP enrichment.
    pub text_preview: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchFilters {
    pub language: Option<String>,
    pub page_version_id: Option<String>,
}

#[async_trait]
pub trait VectorStore: Send + Sync {
    async fn health(&self) -> Result<bool, VectorStoreError>;

    /// Produce chunk records to Chronik. Returns the
    /// `(partition, offset)` for each chunk in input order.
    async fn produce_chunks(
        &self,
        chunks: Vec<ChunkPayload>,
    ) -> Result<Vec<ProducedRecord>, VectorStoreError>;

    /// Top-k semantic search on the topic. The query text is embedded
    /// by Chronik using the topic's configured embedding model.
    async fn search(
        &self,
        query: &str,
        filters: SearchFilters,
        k: usize,
    ) -> Result<Vec<ChunkRef>, VectorStoreError>;
}

pub fn allow_in_memory_vector_store() -> bool {
    std::env::var("ALLOW_IN_MEMORY_VECTOR_STORE")
        .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
        .unwrap_or(false)
}

// ---- InMemoryVectorStore ----

/// In-memory store for unit tests. Substring-matches the query
/// against chunk content and returns synthetic `(partition, offset)`
/// pairs. Not a semantic search — only useful to exercise the
/// upstream code paths.
pub struct InMemoryVectorStore {
    store: RwLock<Vec<(ProducedRecord, ChunkPayload)>>,
    next_offset: RwLock<i64>,
}

impl InMemoryVectorStore {
    pub fn new() -> Self {
        Self {
            store: RwLock::new(Vec::new()),
            next_offset: RwLock::new(0),
        }
    }
}

impl Default for InMemoryVectorStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl VectorStore for InMemoryVectorStore {
    async fn health(&self) -> Result<bool, VectorStoreError> {
        Ok(true)
    }

    async fn produce_chunks(
        &self,
        chunks: Vec<ChunkPayload>,
    ) -> Result<Vec<ProducedRecord>, VectorStoreError> {
        let mut store = self
            .store
            .write()
            .map_err(|e| VectorStoreError::Internal(format!("lock poisoned: {e}")))?;
        let mut next = self
            .next_offset
            .write()
            .map_err(|e| VectorStoreError::Internal(format!("lock poisoned: {e}")))?;

        let mut records = Vec::with_capacity(chunks.len());
        for chunk in chunks {
            let rec = ProducedRecord {
                partition: 0,
                offset: *next,
            };
            *next += 1;
            store.push((rec, chunk));
            records.push(rec);
        }
        Ok(records)
    }

    async fn search(
        &self,
        query: &str,
        filters: SearchFilters,
        k: usize,
    ) -> Result<Vec<ChunkRef>, VectorStoreError> {
        let store = self
            .store
            .read()
            .map_err(|e| VectorStoreError::Internal(format!("lock poisoned: {e}")))?;
        let q = query.to_lowercase();
        let mut hits: Vec<ChunkRef> = store
            .iter()
            .filter(|(_, c)| {
                if let Some(ref lang) = filters.language {
                    if &c.language != lang {
                        return false;
                    }
                }
                if let Some(ref pv) = filters.page_version_id {
                    if &c.page_version_id != pv {
                        return false;
                    }
                }
                true
            })
            .filter_map(|(rec, c)| {
                let lc = c.content.to_lowercase();
                if lc.contains(&q) {
                    Some(ChunkRef {
                        partition: rec.partition,
                        offset: rec.offset,
                        score: 1.0,
                        text_preview: Some(c.content.chars().take(200).collect()),
                    })
                } else {
                    None
                }
            })
            .collect();
        hits.truncate(k);
        Ok(hits)
    }
}

// ---- ChronikVectorStore ----

/// Vector store backed by Chronik-Stream. Writes go through Kafka
/// (`ChronikClient::kafka_producer`), reads go through the REST
/// `/_vector/<topic>/search` endpoint.
pub struct ChronikVectorStore {
    client: crate::chronik::ChronikClient,
}

impl ChronikVectorStore {
    pub fn new(client: crate::chronik::ChronikClient) -> Self {
        Self { client }
    }
}

#[async_trait]
impl VectorStore for ChronikVectorStore {
    async fn health(&self) -> Result<bool, VectorStoreError> {
        self.client.search_health().await
    }

    async fn produce_chunks(
        &self,
        chunks: Vec<ChunkPayload>,
    ) -> Result<Vec<ProducedRecord>, VectorStoreError> {
        use crate::chronik::producer::topics::PUBLISHED_PAGES;

        let mut out = Vec::with_capacity(chunks.len());
        for chunk in chunks {
            let key = format!("{}:{}", chunk.page_version_id, chunk.section_index);
            let payload = serde_json::json!({
                "page_version_id": chunk.page_version_id,
                "section_index": chunk.section_index,
                "heading_path": chunk.heading_path,
                "content": chunk.content,
                "language": chunk.language,
                "token_count": chunk.token_count,
            });
            let rec = self
                .client
                .kafka_producer
                .produce(PUBLISHED_PAGES, &key, &payload)
                .await
                .map_err(|e| VectorStoreError::Kafka(e.to_string()))?;
            out.push(rec);
        }
        Ok(out)
    }

    async fn search(
        &self,
        query: &str,
        filters: SearchFilters,
        k: usize,
    ) -> Result<Vec<ChunkRef>, VectorStoreError> {
        self.client.vector_search_text(query, filters, k).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn in_memory_substring_search() {
        let store = InMemoryVectorStore::new();
        store
            .produce_chunks(vec![
                ChunkPayload {
                    page_version_id: "pv-1".into(),
                    section_index: 0,
                    heading_path: vec!["Intro".into()],
                    content: "Hello world".into(),
                    language: "en".into(),
                    token_count: 2,
                },
                ChunkPayload {
                    page_version_id: "pv-1".into(),
                    section_index: 1,
                    heading_path: vec!["Details".into()],
                    content: "More details".into(),
                    language: "en".into(),
                    token_count: 2,
                },
            ])
            .await
            .unwrap();

        let hits = store.search("hello", SearchFilters::default(), 10).await.unwrap();
        assert_eq!(hits.len(), 1);
    }

    #[tokio::test]
    async fn in_memory_health_returns_true() {
        let store = InMemoryVectorStore::new();
        assert!(store.health().await.unwrap());
    }
}
```

- [ ] **Step 4: Rewrite `crates/db/src/chronik/search.rs`**

Replace its contents with:

```rust
//! Chronik REST search calls. Writes are not here — they go through
//! the Kafka producer in `crate::chronik::kafka_producer`.

use serde::{Deserialize, Serialize};

use super::ChronikClient;
use crate::chronik::producer::topics::PUBLISHED_PAGES;
use crate::vector_store::{ChunkRef, SearchFilters, VectorStoreError};

#[derive(Debug, Serialize)]
struct VectorSearchRequest {
    query: String,
    k: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    filters: Option<SearchFiltersWire>,
}

#[derive(Debug, Serialize)]
struct SearchFiltersWire {
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    page_version_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct VectorSearchResponse {
    results: Vec<VectorSearchHit>,
}

#[derive(Debug, Deserialize)]
struct VectorSearchHit {
    partition: i32,
    offset: i64,
    score: f32,
    #[serde(default)]
    text_preview: Option<String>,
}

impl ChronikClient {
    /// Semantic search via Chronik's `/_vector/<topic>/search` endpoint.
    /// Chronik embeds the query using the topic's configured model.
    pub async fn vector_search_text(
        &self,
        query: &str,
        filters: SearchFilters,
        k: usize,
    ) -> Result<Vec<ChunkRef>, VectorStoreError> {
        let url = format!(
            "{}/_vector/{}/search",
            self.search_base_url, PUBLISHED_PAGES
        );
        let body = VectorSearchRequest {
            query: query.to_string(),
            k,
            filters: if filters.language.is_some() || filters.page_version_id.is_some() {
                Some(SearchFiltersWire {
                    language: filters.language,
                    page_version_id: filters.page_version_id,
                })
            } else {
                None
            },
        };

        let resp = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| VectorStoreError::Internal(format!("chronik search request: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(VectorStoreError::Internal(format!(
                "chronik search HTTP {status}: {text}"
            )));
        }

        let parsed: VectorSearchResponse = resp
            .json()
            .await
            .map_err(|e| VectorStoreError::Internal(format!("chronik search parse: {e}")))?;

        Ok(parsed
            .results
            .into_iter()
            .map(|h| ChunkRef {
                partition: h.partition,
                offset: h.offset,
                score: h.score,
                text_preview: h.text_preview,
            })
            .collect())
    }

    /// Health check against Chronik's `/health` endpoint.
    pub async fn search_health(&self) -> Result<bool, VectorStoreError> {
        let url = format!("{}/health", self.search_base_url);
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(VectorStoreError::Http)?;
        Ok(resp.status().is_success())
    }
}
```

- [ ] **Step 5: Run the e2e test**

Make sure the topic exists first (Task 4 wiring runs only on API boot, but tests can call `ensure_topic` themselves — this test does). Run:

```bash
CHRONIK_KAFKA_BROKER=localhost:9092 cargo test -p historiador_db --test chronik_vector_store_e2e -- --nocapture
```

Expected: PASS within 60s. (If Chronik's embedding pipeline is slow on first run because OpenAI is rate-limited or `OPENAI_API_KEY` is not set on the Chronik container, the test will time out. Set the key on the chronik service via `docker-compose.yml` — Task 11.)

- [ ] **Step 6: Commit**

```bash
git add crates/db/src/vector_store.rs crates/db/src/chronik/search.rs \
        crates/db/tests/chronik_vector_store_e2e.rs
git commit -m "feat(chronik): rewrite VectorStore for Kafka writes + /_vector text search"
```

---

### Task 8: Update `chunks` Postgres module for `(partition, offset)`

**Files:**
- Modify: `crates/db/src/postgres/chunks.rs`
- Modify: `crates/db/src/postgres/mcp_queries.rs`

- [ ] **Step 1: Update `chunks.rs` types and queries**

Replace `crates/db/src/postgres/chunks.rs` with:

```rust
//! Queries against the `chunks` table.
//!
//! Each row references a Chronik record by its `(chronik_partition,
//! chronik_offset)` pair. Embeddings live in Chronik (ADR-007); only
//! the back-pointer is stored in Postgres so MCP enrichment can map a
//! search hit back to its page/collection.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow, serde::Serialize)]
pub struct ChunkRow {
    pub id: Uuid,
    pub page_version_id: Uuid,
    pub heading_path: Vec<String>,
    pub section_index: i32,
    pub token_count: i32,
    pub oversized: bool,
    pub language: String,
    pub chronik_partition: Option<i32>,
    pub chronik_offset: Option<i64>,
    pub created_at: DateTime<Utc>,
}

pub struct NewChunk {
    pub page_version_id: Uuid,
    pub heading_path: Vec<String>,
    pub section_index: i32,
    pub token_count: i32,
    pub oversized: bool,
    pub language: String,
    pub chronik_partition: i32,
    pub chronik_offset: i64,
}

pub async fn insert_batch(pool: &PgPool, chunks: &[NewChunk]) -> anyhow::Result<Vec<ChunkRow>> {
    let mut rows = Vec::with_capacity(chunks.len());
    for chunk in chunks {
        let row = sqlx::query_as::<_, ChunkRow>(
            "INSERT INTO chunks \
               (page_version_id, heading_path, section_index, token_count, \
                oversized, language, chronik_partition, chronik_offset) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
             RETURNING *",
        )
        .bind(chunk.page_version_id)
        .bind(&chunk.heading_path)
        .bind(chunk.section_index)
        .bind(chunk.token_count)
        .bind(chunk.oversized)
        .bind(&chunk.language)
        .bind(chunk.chronik_partition)
        .bind(chunk.chronik_offset)
        .fetch_one(pool)
        .await?;
        rows.push(row);
    }
    Ok(rows)
}

pub async fn delete_by_page_version(pool: &PgPool, page_version_id: Uuid) -> anyhow::Result<u64> {
    let result = sqlx::query("DELETE FROM chunks WHERE page_version_id = $1")
        .bind(page_version_id)
        .execute(pool)
        .await?;
    Ok(result.rows_affected())
}

pub async fn find_by_page_version(
    pool: &PgPool,
    page_version_id: Uuid,
) -> anyhow::Result<Vec<ChunkRow>> {
    let rows = sqlx::query_as::<_, ChunkRow>(
        "SELECT * FROM chunks WHERE page_version_id = $1 ORDER BY section_index",
    )
    .bind(page_version_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}
```

- [ ] **Step 2: Rewrite `enrich_chunk_results` in `mcp_queries.rs`**

Open `crates/db/src/postgres/mcp_queries.rs`. Find the `enrich_chunk_results` function (it currently takes `&[Uuid]` page-version ids and joins through pages/collections). Replace it with a function keyed on `(partition, offset)`:

```rust
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct EnrichedChunk {
    pub page_version_id: Uuid,
    pub page_id: Uuid,
    pub collection_id: Option<Uuid>,
    pub page_title: String,
    pub language: String,
    pub collection_path: Vec<String>,
    pub heading_path: Vec<String>,
    pub content_markdown: String,
}

/// Look up enrichment for a list of `(partition, offset)` pairs.
/// Returns a map keyed on the same pairs. Pairs without a matching
/// `chunks` row are absent from the map (Chronik may return orphans
/// from prior reindex generations).
pub async fn enrich_chunk_results(
    pool: &sqlx::PgPool,
    refs: &[(i32, i64)],
) -> anyhow::Result<HashMap<(i32, i64), EnrichedChunk>> {
    if refs.is_empty() {
        return Ok(HashMap::new());
    }

    // Build a VALUES list so the query is one round-trip.
    let mut sql = String::from(
        "WITH refs(partition, offset) AS (VALUES ",
    );
    for i in 0..refs.len() {
        if i > 0 {
            sql.push(',');
        }
        sql.push_str(&format!("(${}::int, ${}::bigint)", i * 2 + 1, i * 2 + 2));
    }
    sql.push_str(
        "), \
         coll_path AS ( \
            SELECT id, ARRAY[name] AS path FROM collections WHERE parent_id IS NULL \
            UNION ALL \
            SELECT c.id, cp.path || c.name FROM collections c \
              JOIN coll_path cp ON c.parent_id = cp.id \
         ) \
         SELECT \
            ch.chronik_partition AS partition, \
            ch.chronik_offset    AS offset, \
            ch.page_version_id, \
            ch.heading_path, \
            pv.language, \
            p.id   AS page_id, \
            p.title AS page_title, \
            p.collection_id, \
            COALESCE(cp.path, '{}') AS collection_path, \
            pv.content_markdown \
         FROM refs r \
         JOIN chunks ch \
            ON ch.chronik_partition = r.partition \
           AND ch.chronik_offset    = r.offset \
         JOIN page_versions pv ON pv.id = ch.page_version_id \
         JOIN pages         p  ON p.id  = pv.page_id \
         LEFT JOIN coll_path cp ON cp.id = p.collection_id",
    );

    let mut q = sqlx::query(&sql);
    for (p, o) in refs {
        q = q.bind(*p).bind(*o);
    }
    let rows = q.fetch_all(pool).await?;

    use sqlx::Row;
    let mut map = HashMap::with_capacity(rows.len());
    for row in rows {
        let partition: i32 = row.get("partition");
        let offset: i64 = row.get("offset");
        map.insert(
            (partition, offset),
            EnrichedChunk {
                page_version_id: row.get("page_version_id"),
                page_id: row.get("page_id"),
                collection_id: row.try_get("collection_id").ok(),
                page_title: row.get("page_title"),
                language: row.get("language"),
                collection_path: row.get("collection_path"),
                heading_path: row.get("heading_path"),
                content_markdown: row.get("content_markdown"),
            },
        );
    }
    Ok(map)
}
```

(Verify the column name `content_markdown` against the actual `page_versions` schema; adjust if the column is named differently. The exact name matters because the MCP server now relies on Postgres for the chunk text rather than Chronik's truncated `text_preview`.)

- [ ] **Step 3: Build and fix any cascading errors**

```bash
cargo build -p historiador_db
```

Fix compile errors as they appear. Common ones: tests in `crates/db/tests/` that reference `vexfs_ref` or the old `enrich_chunk_results` signature.

- [ ] **Step 4: Commit**

```bash
git add crates/db/src/postgres/chunks.rs crates/db/src/postgres/mcp_queries.rs
git commit -m "refactor(db): chunks reference Chronik via (partition, offset); enrich on that key"
```

---

### Task 9: Update the chunk pipeline (full reindex, no embedding step)

**Files:**
- Modify: `apps/api/src/infrastructure/chunker/pipeline.rs`
- Modify: `apps/api/src/state.rs` (drop `embedding_client` from chunk-pipeline construction if it's wired here)

- [ ] **Step 1: Replace the pipeline implementation**

Replace `apps/api/src/infrastructure/chunker/pipeline.rs` with:

```rust
//! Chunk pipeline — splits the published markdown into chunks,
//! produces them to Chronik via Kafka, and stores the back-pointer
//! `(partition, offset)` in Postgres.
//!
//! Republish is a full reindex: existing `chunks` rows for the page
//! version are deleted, then all new chunks are produced and stored.
//! Chronik records produced before the reindex become orphans (no
//! Postgres row); MCP enrichment skips them.

use std::sync::Arc;

use async_trait::async_trait;
use sqlx::PgPool;
use uuid::Uuid;

use historiador_chunker::{chunk_markdown, ChunkConfig};
use historiador_db::postgres::chunks;
use historiador_db::vector_store::{ChunkPayload, VectorStore};

use crate::domain::error::ApplicationError;
use crate::domain::port::chunk_pipeline::{ChunkPipeline, ChunkPipelineInput};

pub struct DefaultChunkPipeline {
    pool: PgPool,
    vector_store: Arc<dyn VectorStore>,
}

impl DefaultChunkPipeline {
    pub fn new(pool: PgPool, vector_store: Arc<dyn VectorStore>) -> Self {
        Self { pool, vector_store }
    }
}

#[async_trait]
impl ChunkPipeline for DefaultChunkPipeline {
    async fn run(&self, input: ChunkPipelineInput) -> Result<(), ApplicationError> {
        let ChunkPipelineInput {
            page_version_id,
            language,
            markdown,
        } = input;

        // Full reindex: delete existing chunks rows for this page
        // version. Chronik records become orphans and are filtered out
        // by MCP enrichment (no matching Postgres row).
        chunks::delete_by_page_version(&self.pool, page_version_id).await?;

        let raw_chunks = match chunk_markdown(&markdown, &ChunkConfig::default()) {
            Ok(c) => c,
            Err(historiador_chunker::ChunkError::EmptyInput) => {
                tracing::warn!(%page_version_id, "empty content, skipping chunk pipeline");
                return Ok(());
            }
        };
        if raw_chunks.is_empty() {
            tracing::warn!(%page_version_id, "chunker produced no chunks");
            return Ok(());
        }

        let payloads: Vec<ChunkPayload> = raw_chunks
            .iter()
            .map(|c| ChunkPayload {
                page_version_id: page_version_id.to_string(),
                section_index: c.section_index as i32,
                heading_path: c.heading_path.clone(),
                content: c.content.clone(),
                language: language.as_str().to_string(),
                token_count: c.token_count as i32,
            })
            .collect();

        let produced = self
            .vector_store
            .produce_chunks(payloads)
            .await
            .map_err(|e| anyhow::anyhow!("chronik produce failed: {e}"))?;

        let new_chunks: Vec<chunks::NewChunk> = raw_chunks
            .iter()
            .zip(produced.iter())
            .map(|(c, rec)| chunks::NewChunk {
                page_version_id,
                heading_path: c.heading_path.clone(),
                section_index: c.section_index as i32,
                token_count: c.token_count as i32,
                oversized: c.oversized,
                language: language.as_str().to_string(),
                chronik_partition: rec.partition,
                chronik_offset: rec.offset,
            })
            .collect();

        chunks::insert_batch(&self.pool, &new_chunks).await?;

        tracing::info!(
            %page_version_id,
            chunk_count = new_chunks.len(),
            "chunk pipeline complete"
        );
        Ok(())
    }

    async fn clear(&self, page_version_id: Uuid) -> Result<(), ApplicationError> {
        // No equivalent in Chronik — orphans are tolerated by design.
        chunks::delete_by_page_version(&self.pool, page_version_id).await?;
        Ok(())
    }
}
```

- [ ] **Step 2: Update construction sites**

Search for `DefaultChunkPipeline::new` callers — likely in `apps/api/src/state.rs`. Drop the `embedding_client` argument there. Example:

Before:
```rust
let chunk_pipeline = Arc::new(DefaultChunkPipeline::new(
    pool.clone(),
    vector_store.clone(),
    embedding_client.clone(),
));
```

After:
```rust
let chunk_pipeline = Arc::new(DefaultChunkPipeline::new(pool.clone(), vector_store.clone()));
```

(The `embedding_client` field on `AppState` may still be needed by other features — leave it where it’s used; just remove it from the chunk-pipeline construction.)

- [ ] **Step 3: Build the API crate**

```bash
cargo build -p historiador_api
```

Fix any cascading compile errors.

- [ ] **Step 4: Commit**

```bash
git add apps/api/src/infrastructure/chunker/pipeline.rs apps/api/src/state.rs
git commit -m "refactor(api): chunk pipeline produces to Chronik via Kafka, drops embedding step"
```

---

### Task 10: Update MCP search use case + enrichment port

**Files:**
- Modify: `apps/mcp/src/application/port.rs`
- Modify: `apps/mcp/src/application/search_chunks.rs`
- Modify: `apps/mcp/src/infrastructure/postgres_readonly.rs`
- Modify: `apps/mcp/src/main.rs`

- [ ] **Step 1: Rewrite `ChunkMetadataReader` port**

In `apps/mcp/src/application/port.rs`, replace the `ChunkMetadataReader` trait + `ChunkMetadata` struct with:

```rust
use std::collections::HashMap;
use async_trait::async_trait;
use uuid::Uuid;

use crate::application::McpError;

#[derive(Debug, Clone)]
pub struct ChunkMetadata {
    pub page_id: Uuid,
    pub page_version_id: Uuid,
    pub collection_id: Option<Uuid>,
    pub page_title: String,
    pub language: String,
    pub collection_path: Vec<String>,
    /// Heading path from the chunk record itself.
    pub heading_path: Vec<String>,
    /// Authoritative chunk content from `page_versions.content_markdown`
    /// (Postgres). Chronik's `text_preview` is truncated and not used
    /// for the response body.
    pub content_markdown: String,
}

#[async_trait]
pub trait ChunkMetadataReader: Send + Sync {
    /// Look up enrichment for a list of `(partition, offset)` pairs.
    /// Pairs missing from the map should be skipped by the caller —
    /// they are orphans from prior reindex generations.
    async fn enrich_many(
        &self,
        refs: &[(i32, i64)],
    ) -> Result<HashMap<(i32, i64), ChunkMetadata>, McpError>;
}
```

- [ ] **Step 2: Adapt `PostgresChunkMetadataReader`**

Replace `apps/mcp/src/infrastructure/postgres_readonly.rs` with:

```rust
use std::collections::HashMap;

use async_trait::async_trait;
use sqlx::PgPool;

use historiador_db::postgres::mcp_queries;

use crate::application::port::{ChunkMetadata, ChunkMetadataReader};
use crate::application::McpError;

pub struct PostgresChunkMetadataReader {
    pool: PgPool,
}

impl PostgresChunkMetadataReader {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ChunkMetadataReader for PostgresChunkMetadataReader {
    async fn enrich_many(
        &self,
        refs: &[(i32, i64)],
    ) -> Result<HashMap<(i32, i64), ChunkMetadata>, McpError> {
        let raw = mcp_queries::enrich_chunk_results(&self.pool, refs).await?;
        Ok(raw
            .into_iter()
            .map(|((p, o), m)| {
                (
                    (p, o),
                    ChunkMetadata {
                        page_id: m.page_id,
                        page_version_id: m.page_version_id,
                        collection_id: m.collection_id,
                        page_title: m.page_title,
                        language: m.language,
                        collection_path: m.collection_path,
                        heading_path: m.heading_path,
                        content_markdown: m.content_markdown,
                    },
                )
            })
            .collect())
    }
}
```

- [ ] **Step 3: Rewrite `SearchChunksUseCase`**

Replace `apps/mcp/src/application/search_chunks.rs` with:

```rust
//! Semantic-search use case: query string → Chronik vector search →
//! Postgres metadata enrichment → `SearchChunksResult`.
//!
//! The `EmbeddingClient` is no longer involved on the read side —
//! Chronik embeds the query using the topic's configured model so
//! both write and read sides agree on the embedding space.

use std::sync::Arc;

use historiador_db::vector_store::{SearchFilters, VectorStore};

use super::port::ChunkMetadataReader;
use super::McpError;

#[derive(Debug, Clone)]
pub struct SearchChunksCommand {
    pub query: String,
    pub language: Option<String>,
    pub top_k: usize,
}

#[derive(Debug, Clone)]
pub struct SearchChunkResult {
    pub content: String,
    pub heading_path: Vec<String>,
    pub page_title: String,
    pub collection_path: Vec<String>,
    pub score: f32,
    pub language: String,
}

#[derive(Debug, Clone)]
pub struct SearchChunksResult {
    pub chunks: Vec<SearchChunkResult>,
    pub language_filter_applied: bool,
}

pub struct SearchChunksUseCase {
    vector_store: Arc<dyn VectorStore>,
    metadata: Arc<dyn ChunkMetadataReader>,
}

impl SearchChunksUseCase {
    pub fn new(vector_store: Arc<dyn VectorStore>, metadata: Arc<dyn ChunkMetadataReader>) -> Self {
        Self { vector_store, metadata }
    }

    pub async fn execute(&self, cmd: SearchChunksCommand) -> Result<SearchChunksResult, McpError> {
        let language_filter_applied = cmd.language.is_some();
        let top_k = cmd.top_k.clamp(1, 20);

        let filters = SearchFilters {
            language: cmd.language,
            ..Default::default()
        };

        let hits = self
            .vector_store
            .search(&cmd.query, filters, top_k)
            .await
            .map_err(|e| anyhow::anyhow!("chronik search failed: {e}"))?;

        if hits.is_empty() {
            return Ok(SearchChunksResult { chunks: vec![], language_filter_applied });
        }

        let refs: Vec<(i32, i64)> = hits.iter().map(|h| (h.partition, h.offset)).collect();
        let meta_map = self.metadata.enrich_many(&refs).await?;

        let chunks = hits
            .into_iter()
            .filter_map(|h| {
                let key = (h.partition, h.offset);
                let m = meta_map.get(&key)?;
                Some(SearchChunkResult {
                    content: m.content_markdown.clone(),
                    heading_path: m.heading_path.clone(),
                    page_title: m.page_title.clone(),
                    collection_path: m.collection_path.clone(),
                    score: h.score,
                    language: m.language.clone(),
                })
            })
            .collect();

        Ok(SearchChunksResult { chunks, language_filter_applied })
    }
}
```

- [ ] **Step 4: Update MCP `main.rs` wiring**

In `apps/mcp/src/main.rs`, find where `SearchChunksUseCase::new(...)` is constructed. Drop the `embedding_client` argument. The `EmbeddingClient` may now be entirely unused on the MCP side — if so, also drop the env-driven `embedding_client` setup.

- [ ] **Step 5: Build the MCP crate**

```bash
cargo build -p historiador_mcp
```

Fix cascading compile errors (handler, query.rs response shape, etc.).

- [ ] **Step 6: Commit**

```bash
git add apps/mcp/src/application/port.rs apps/mcp/src/application/search_chunks.rs \
        apps/mcp/src/infrastructure/postgres_readonly.rs apps/mcp/src/main.rs
git commit -m "refactor(mcp): search by text via Chronik, enrich by (partition, offset)"
```

---

### Task 11: Pass `OPENAI_API_KEY` to the Chronik container

**Files:**
- Modify: `docker-compose.yml`
- Modify: `.env.example`

- [ ] **Step 1: Inspect the existing `chronik` service block**

Run:

```bash
grep -nA 15 "chronik:" docker-compose.yml | head -30
```

- [ ] **Step 2: Add the env var**

In `docker-compose.yml`, under the `chronik` service `environment:` block, add:

```yaml
      OPENAI_API_KEY: ${EMBEDDING_API_KEY:-}
      CHRONIK_EMBEDDING_API_KEY: ${EMBEDDING_API_KEY:-}
```

(Both names are accepted by Chronik per VECTOR_SEARCH_GUIDE.md "Set Your API Key". Setting both is harmless.)

- [ ] **Step 3: Document the prereq in `.env.example`**

In `.env.example`, near the existing `# EMBEDDING_API_KEY=sk-...` line, add the comment:

```
# EMBEDDING_API_KEY is required for Chronik's built-in embedding pipeline
# on the published-pages topic. Without it, produces will succeed but
# the vector index will not be populated and MCP search will return no
# hits. Set this in .env BEFORE bringing up docker-compose.
```

- [ ] **Step 4: Restart Chronik with the key**

Run:

```bash
docker compose up -d chronik
docker exec historiador-doc-chronik-1 env | grep -i embedding
```

Expected: `CHRONIK_EMBEDDING_API_KEY=sk-...` and/or `OPENAI_API_KEY=sk-...`.

- [ ] **Step 5: Commit**

```bash
git add docker-compose.yml .env.example
git commit -m "chore(docker): pass embedding API key to Chronik for vector ingest"
```

---

### Task 12: Fix the analytics SQL endpoint path

**Files:**
- Modify: `crates/db/src/chronik/analytics.rs`

- [ ] **Step 1: Fix the URL**

In `crates/db/src/chronik/analytics.rs:59`, replace:

```rust
let url = format!("{}/api/v1/sql", self.base_url);
```

with:

```rust
let url = format!("{}/_sql", self.base_url);
```

- [ ] **Step 2: Build to make sure nothing else relied on the old path**

```bash
cargo build -p historiador_db
```

- [ ] **Step 3: Commit**

```bash
git add crates/db/src/chronik/analytics.rs
git commit -m "fix(chronik): SQL endpoint is /_sql, not /api/v1/sql"
```

---

### Task 13: Drop the dead `HttpVexfsClient`

**Files:**
- Modify: `crates/db/src/vector_store.rs` (already partly handled in Task 7)

- [ ] **Step 1: Confirm no callers**

```bash
grep -rn "HttpVexfsClient" crates apps
```

Expected: no results (Task 7 already removed it from `vector_store.rs`; this step verifies nothing else imports it).

- [ ] **Step 2: If grep returns hits, delete those imports**

For each remaining hit, remove the `use ... HttpVexfsClient` import and any code constructing it. (Likely zero hits — Task 7 should have nuked it.)

- [ ] **Step 3: Commit if anything changed**

```bash
git status --short
# If any files in red:
git add -u
git commit -m "chore(db): remove dead HttpVexfsClient references"
```

(If nothing changed, skip the commit.)

---

### Task 14: End-to-end MCP query test

**Files:**
- Create: `apps/mcp/tests/mcp_query_e2e.rs`

- [ ] **Step 1: Write the test**

This test boots both the API and the MCP server in-process, publishes a page through the API, polls until Chronik has indexed it, then runs the MCP `query` tool and asserts the chunk appears in the result.

The test envelope mirrors `apps/api/tests/sprint3_e2e.rs` for setup. Create `apps/mcp/tests/mcp_query_e2e.rs`:

```rust
//! End-to-end: publish a page through the API, then query it through
//! the MCP server. Requires real Chronik on :9092 and Postgres on
//! :5432 (i.e. `docker compose up -d`).
//!
//! Skipped if `CHRONIK_KAFKA_BROKER` is unset.

use std::time::{Duration, Instant};

use serde_json::json;

const POLL_TIMEOUT: Duration = Duration::from_secs(120);

fn skip_if_no_chronik() -> Option<String> {
    std::env::var("CHRONIK_KAFKA_BROKER").ok()
}

#[tokio::test]
async fn publish_then_mcp_query_returns_the_chunk() {
    let Some(_broker) = skip_if_no_chronik() else {
        eprintln!("skipping: CHRONIK_KAFKA_BROKER not set");
        return;
    };
    if std::env::var("EMBEDDING_API_KEY").is_err() {
        eprintln!("skipping: EMBEDDING_API_KEY required for Chronik to embed");
        return;
    }

    // 1. Boot the API and MCP servers in-process. Reuse the helpers
    //    that sprint3_e2e.rs uses; if those aren't pub, copy the
    //    setup verbatim. (Pseudocode here — fill in with whatever the
    //    existing test harness exposes.)
    let api_app = historiador_api::testing::build_test_app().await;
    let mcp_app = historiador_mcp::testing::build_test_app(api_app.pool.clone()).await;

    // 2. Run setup wizard with the "test" llm_provider and a real
    //    OpenAI embedding provider (so Chronik can embed). If the
    //    wizard requires an MCP token, capture it.
    let setup_resp = api_app
        .request_json(
            "POST",
            "/setup/init",
            json!({
                "workspace_name": "e2e",
                "language": "en",
                "llm_provider": "test",
            }),
        )
        .await;
    let admin_token: String = setup_resp["jwt"].as_str().unwrap().to_string();

    // 3. Create a collection, then a page with distinctive content.
    let collection = api_app
        .request_json_auth(
            "POST",
            "/collections",
            &admin_token,
            json!({"name": "e2e-coll"}),
        )
        .await;
    let collection_id = collection["id"].as_str().unwrap();

    let page = api_app
        .request_json_auth(
            "POST",
            "/pages",
            &admin_token,
            json!({
                "title": "E2E test page",
                "collection_id": collection_id,
                "markdown": "# Indexing test\n\nThe quick brown fox jumps over the lazy dog. \
                             This page exists to verify Chronik vector indexing end-to-end.",
                "language": "en",
            }),
        )
        .await;
    let page_id = page["id"].as_str().unwrap();

    // 4. Publish.
    api_app
        .request_json_auth(
            "POST",
            &format!("/pages/{page_id}/publish"),
            &admin_token,
            json!({}),
        )
        .await;

    // 5. Poll the MCP `query` until it returns a hit, or timeout.
    let mcp_token = api_app
        .request_json_auth("POST", "/mcp/tokens", &admin_token, json!({}))
        .await;
    let mcp_bearer = mcp_token["token"].as_str().unwrap();

    let deadline = Instant::now() + POLL_TIMEOUT;
    let mut last_err = String::new();
    while Instant::now() < deadline {
        let mcp_resp = mcp_app
            .request_json_auth(
                "POST",
                "/mcp",
                mcp_bearer,
                json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "method": "tools/call",
                    "params": {
                        "name": "query",
                        "arguments": {"query": "fox dog jump", "top_k": 5}
                    }
                }),
            )
            .await;
        let chunks = &mcp_resp["result"]["content"];
        if chunks.is_array() && !chunks.as_array().unwrap().is_empty() {
            // Found a hit. Sanity-check it references our page.
            let body = chunks[0]["text"].as_str().unwrap_or("");
            assert!(
                body.contains("fox") || body.contains("dog"),
                "expected our test text in the chunk body, got: {body}"
            );
            return;
        }
        last_err = format!("no chunks yet: {mcp_resp}");
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    panic!("timeout waiting for chunk to be indexed and queryable. last response: {last_err}");
}
```

- [ ] **Step 2: Run the failing test (no harness yet, possibly)**

```bash
CHRONIK_KAFKA_BROKER=localhost:9092 EMBEDDING_API_KEY=$EMBEDDING_API_KEY \
  cargo test -p historiador_mcp --test mcp_query_e2e -- --nocapture
```

If `historiador_api::testing::build_test_app` and `historiador_mcp::testing::build_test_app` don't exist as `pub`, you'll get a compile error. Address by either:
- Promoting the test harness from `sprint3_e2e.rs` into `pub mod testing` in `apps/api/src/lib.rs` (and same for MCP), OR
- Copying the setup code inline into this test.

Pick the option that touches fewest files. Promotion is cleaner long-term.

- [ ] **Step 3: Run again, expect PASS**

```bash
CHRONIK_KAFKA_BROKER=localhost:9092 EMBEDDING_API_KEY=$EMBEDDING_API_KEY \
  cargo test -p historiador_mcp --test mcp_query_e2e -- --nocapture
```

Expected: PASS within 120s. The pipeline now actually indexes chunks; MCP returns the published chunk.

If it times out, the most likely failure modes are (a) Chronik can't reach OpenAI (check `docker logs historiador-doc-chronik-1` for embedding errors), (b) the `published-pages` topic was created without `vector.enabled=true` (delete and re-create: `kafkactl delete topic published-pages` via the chronik admin client, then restart the API to re-provision). Diagnose before changing test timeout.

- [ ] **Step 4: Commit**

```bash
git add apps/mcp/tests/mcp_query_e2e.rs apps/api/src/lib.rs apps/mcp/src/lib.rs
git commit -m "test(mcp): end-to-end publish → MCP query against real Chronik"
```

---

### Task 15: Sweep — clippy, fmt, full test pass

**Files:** all touched files.

- [ ] **Step 1: Format**

```bash
cargo fmt --all
```

- [ ] **Step 2: Clippy (CI denies warnings)**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Fix every warning. Common ones from this rewrite: unused `EmbeddingClient` imports, unused `Uuid` imports, dead helper functions left over from the old code path.

- [ ] **Step 3: Run the workspace test suite (without Chronik tests)**

```bash
cargo test --workspace
```

Tests that gate on `CHRONIK_KAFKA_BROKER` get skipped without it set. All other tests should pass.

- [ ] **Step 4: Run the workspace test suite WITH Chronik**

```bash
CHRONIK_KAFKA_BROKER=localhost:9092 EMBEDDING_API_KEY=$EMBEDDING_API_KEY \
  cargo test --workspace -- --nocapture
```

All tests should pass, including the new e2e tests.

- [ ] **Step 5: Commit any fixes**

```bash
git status --short
git add -u
git commit -m "chore: clippy/fmt sweep after chronik rewrite"
```

(If nothing changed, skip.)

---

## Self-Review

**Spec coverage**

| Requirement | Task |
|---|---|
| Replace fictional `/api/v1/upsert` with real Kafka write path | Tasks 2, 3, 5, 7 |
| Replace `/api/v1/topics/.../produce` (page-events) | Task 6 |
| Replace `/api/v1/sql` with `/_sql` | Task 12 |
| Replace `/api/v1/version` (HttpVexfsClient was the dead caller) | Task 7 (drop the type) + Task 13 (sweep) |
| Hit→chunk mapping via `(partition, offset)` | Tasks 1, 8, 10 |
| Full reindex on republish | Task 9 |
| Fire-and-forget produce stays | Task 6 (wrapper unchanged) |
| Pure-Rust Kafka client | Task 2 (rskafka) |
| End-to-end test that catches this class of bug | Task 14 |
| Chronik gets the OpenAI key it needs to embed | Task 11 |
| Workspace clippy/fmt clean | Task 15 |

**Placeholder scan:** searched for "TBD" / "implement later" — none. Each step that requires code has the code inline. The only conditional spots are (a) Step 5 of Task 3 says "if rskafka 0.5 doesn't have ... fall back to" — this is a real fallback for a real library uncertainty, not a placeholder; (b) Task 14 Step 2 mentions "promote test harness OR copy inline" because the existing harness in `sprint3_e2e.rs` may not be `pub` — both options are fully described.

**Type consistency:** `ProducedRecord { partition: i32, offset: i64 }` is consistent across `kafka_producer`, `vector_store::ChunkRef`, `chunks::NewChunk` (`chronik_partition: i32, chronik_offset: i64`), `mcp_queries::enrich_chunk_results` (key `(i32, i64)`), and `ChunkMetadataReader::enrich_many` (refs `&[(i32, i64)]`). `ChunkPayload` (write side) and `ChunkRef` (read side) have distinct names so the directional difference is clear.

---

## Execution Handoff

Plan complete and saved to `artifacts/plans/2026-04-25-chronik-vector-store-rewrite.md`. Two execution options:

**1. Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration.

**2. Inline Execution** — Execute tasks in this session using executing-plans, batch execution with checkpoints.

Which approach?
