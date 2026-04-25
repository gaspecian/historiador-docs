//! Pure-Rust Kafka producer for Chronik-Stream writes.
//!
//! Chronik 2.4.1 has no HTTP write path. All ingest must go through
//! the Kafka wire protocol on port 9092. This wrapper exposes a
//! produce-and-await-offset call plus topic provisioning.
//!
//! # Topic config note
//!
//! `rskafka` 0.5.0's `ControllerClient::create_topic` does not accept a
//! per-topic config map and there is no `alter_resource_configs` method.
//! Chronik-specific topic configuration (e.g. `vector.enabled=true`) must
//! be applied out-of-band — either via `kafka-configs.sh` or via Chronik's
//! REST admin API. The `ensure_topic` method accepts an optional
//! `topic_config` parameter for forward-compatibility but silently ignores
//! it at this layer. Task 4 handles Chronik topic config via REST.
//!
//! # Performance note
//!
//! `KafkaProducer` caches `PartitionClient` instances keyed by `(topic,
//! partition)` so that repeated `produce` calls (e.g. streaming chunks for a
//! single page) incur only one metadata round-trip per (topic, partition)
//! pair. The cache is shared across all clones of a `KafkaProducer` via an
//! inner `Arc`.

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use rskafka::client::partition::{Compression, PartitionClient, UnknownTopicHandling};
use rskafka::client::{Client, ClientBuilder};
use rskafka::record::Record;
use serde_json::Value;
use thiserror::Error;
use tokio::sync::Mutex;

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

impl From<rskafka::client::error::Error> for KafkaProducerError {
    fn from(e: rskafka::client::error::Error) -> Self {
        KafkaProducerError::Client(e.to_string())
    }
}

/// The address of a produced record inside a Chronik topic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProducedRecord {
    pub partition: i32,
    pub offset: i64,
}

/// Shared, clone-safe inner state for `KafkaProducer`.
struct Inner {
    client: Arc<Client>,
    /// Cached partition clients keyed by `(topic_name, partition_index)`.
    /// Populated lazily on first `produce` call for each (topic, partition).
    partition_clients: Mutex<HashMap<(String, i32), Arc<PartitionClient>>>,
    /// Cached partition counts keyed by topic name.
    /// Populated lazily on first `produce` call for each topic.
    partition_counts: Mutex<HashMap<String, i32>>,
}

#[derive(Clone)]
pub struct KafkaProducer {
    inner: Arc<Inner>,
}

impl KafkaProducer {
    /// Connect to a Kafka broker and return a ready-to-use producer.
    pub async fn connect(broker: &str) -> Result<Self, KafkaProducerError> {
        let client = ClientBuilder::new(vec![broker.to_owned()])
            .build()
            .await
            .map_err(|e| KafkaProducerError::Client(e.to_string()))?;

        Ok(Self {
            inner: Arc::new(Inner {
                client: Arc::new(client),
                partition_clients: Mutex::new(HashMap::new()),
                partition_counts: Mutex::new(HashMap::new()),
            }),
        })
    }

    /// Ensure a topic exists with at least `partitions` partitions.
    ///
    /// This is idempotent: if the topic already exists (including a race where
    /// two callers both attempt creation concurrently), no error is returned.
    ///
    /// The `topic_config` argument is accepted for API forward-compatibility
    /// but is not applied at this layer — `rskafka` 0.5.0 does not expose
    /// `alter_resource_configs`. Chronik-specific config must be applied via
    /// the Chronik REST admin API (handled in Task 4).
    pub async fn ensure_topic(
        &self,
        topic: &str,
        partitions: i32,
        _topic_config: Option<BTreeMap<String, String>>,
    ) -> Result<(), KafkaProducerError> {
        // Check if the topic already exists.
        let existing = self
            .inner
            .client
            .list_topics()
            .await
            .map_err(|e| KafkaProducerError::Admin(e.to_string()))?;

        let already_exists = existing.iter().any(|t| t.name == topic);
        if already_exists {
            return Ok(());
        }

        // Create the topic with replication_factor=1 (single-broker dev setup).
        let controller = self
            .inner
            .client
            .controller_client()
            .map_err(|e| KafkaProducerError::Admin(e.to_string()))?;

        match controller
            .create_topic(
                topic,
                partitions,
                /*replication_factor*/ 1i16,
                /*timeout_ms*/ 5_000i32,
            )
            .await
        {
            Ok(_) => {}
            Err(e) => {
                let msg = e.to_string();
                // Two concurrent callers may both pass the `list_topics` check
                // above; treat "already exists" from the broker as success.
                if msg.contains("TOPIC_ALREADY_EXISTS")
                    || msg.contains("TopicAlreadyExistsException")
                    || msg.to_lowercase().contains("already exists")
                {
                    // Idempotent — another caller won the race; topic exists.
                } else {
                    return Err(KafkaProducerError::Admin(msg));
                }
            }
        }

        Ok(())
    }

    /// Produce a single JSON payload to a topic and return its (partition, offset).
    ///
    /// The target partition is selected by hashing the key modulo the number of
    /// partitions advertised in cluster metadata. For single-partition topics this
    /// always yields partition 0.
    ///
    /// `PartitionClient` instances are cached per `(topic, partition)` pair so
    /// that repeated calls (e.g. streaming chunks for a page) incur at most one
    /// metadata round-trip per pair rather than one per call.
    pub async fn produce(
        &self,
        topic: &str,
        key: &str,
        payload: &Value,
    ) -> Result<ProducedRecord, KafkaProducerError> {
        let value_bytes = serde_json::to_vec(payload)?;

        // Resolve partition count from cache; fetch from broker only on miss.
        let num_partitions = {
            let mut counts = self.inner.partition_counts.lock().await;
            if let Some(&n) = counts.get(topic) {
                n
            } else {
                let topics = self
                    .inner
                    .client
                    .list_topics()
                    .await
                    .map_err(|e| KafkaProducerError::Produce(e.to_string()))?;

                let n = topics
                    .iter()
                    .find(|t| t.name == topic)
                    .map(|t| t.partitions.len() as i32)
                    .unwrap_or(1)
                    .max(1);

                counts.insert(topic.to_owned(), n);
                n
            }
        };

        let partition = select_partition(key, num_partitions);

        // Resolve partition client from cache; create only on miss.
        let partition_client = {
            let cache_key = (topic.to_owned(), partition);
            let mut clients = self.inner.partition_clients.lock().await;
            if let Some(pc) = clients.get(&cache_key) {
                Arc::clone(pc)
            } else {
                let pc = self
                    .inner
                    .client
                    .partition_client(topic, partition, UnknownTopicHandling::Retry)
                    .await
                    .map_err(|e| KafkaProducerError::Produce(e.to_string()))?;
                let pc = Arc::new(pc);
                clients.insert(cache_key, Arc::clone(&pc));
                pc
            }
        };

        let record = Record {
            key: Some(key.as_bytes().to_vec()),
            value: Some(value_bytes),
            headers: BTreeMap::new(),
            timestamp: Utc::now(),
        };

        let offsets = partition_client
            .produce(vec![record], Compression::Snappy)
            .await
            .map_err(|e| KafkaProducerError::Produce(e.to_string()))?;

        // produce() returns one offset per record; we sent exactly one.
        let offset = offsets
            .into_iter()
            .next()
            .ok_or_else(|| KafkaProducerError::Produce("no offset returned".to_owned()))?;

        Ok(ProducedRecord { partition, offset })
    }
}

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

/// Select a partition index by hashing the key.
///
/// Uses [`std::collections::hash_map::DefaultHasher`] (stable, no extra deps).
/// For single-partition topics this always returns 0.
fn select_partition(key: &str, num_partitions: i32) -> i32 {
    use std::hash::{Hash, Hasher};

    if num_partitions <= 1 {
        return 0;
    }
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut hasher);
    let h = hasher.finish();
    (h % num_partitions as u64) as i32
}
