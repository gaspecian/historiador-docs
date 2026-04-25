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

use std::collections::BTreeMap;
use std::hash::Hash;
use std::sync::Arc;

use chrono::Utc;
use rskafka::client::partition::{Compression, UnknownTopicHandling};
use rskafka::client::{Client, ClientBuilder};
use rskafka::record::Record;
use serde_json::Value;
use thiserror::Error;

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

#[derive(Clone)]
pub struct KafkaProducer {
    client: Arc<Client>,
}

impl KafkaProducer {
    /// Connect to a Kafka broker and return a ready-to-use producer.
    pub async fn connect(broker: &str) -> Result<Self, KafkaProducerError> {
        let client = ClientBuilder::new(vec![broker.to_owned()])
            .build()
            .await
            .map_err(|e| KafkaProducerError::Client(e.to_string()))?;

        Ok(Self {
            client: Arc::new(client),
        })
    }

    /// Ensure a topic exists with at least `partitions` partitions.
    ///
    /// This is idempotent: if the topic already exists, no error is returned.
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
            .client
            .controller_client()
            .map_err(|e| KafkaProducerError::Admin(e.to_string()))?;

        controller
            .create_topic(
                topic,
                partitions,
                /*replication_factor*/ 1i16,
                /*timeout_ms*/ 5_000i32,
            )
            .await
            .map_err(|e| KafkaProducerError::Admin(e.to_string()))?;

        Ok(())
    }

    /// Produce a single JSON payload to a topic and return its (partition, offset).
    ///
    /// The target partition is selected by hashing the key modulo the number of
    /// partitions advertised in cluster metadata. For single-partition topics this
    /// always yields partition 0.
    pub async fn produce(
        &self,
        topic: &str,
        key: &str,
        payload: &Value,
    ) -> Result<ProducedRecord, KafkaProducerError> {
        let value_bytes = serde_json::to_vec(payload)?;

        // Resolve how many partitions this topic has so we can hash-select one.
        let topics = self
            .client
            .list_topics()
            .await
            .map_err(|e| KafkaProducerError::Produce(e.to_string()))?;

        let num_partitions = topics
            .iter()
            .find(|t| t.name == topic)
            .map(|t| t.partitions.len() as i32)
            .unwrap_or(1)
            .max(1);

        let partition = select_partition(key, num_partitions);

        let record = Record {
            key: Some(key.as_bytes().to_vec()),
            value: Some(value_bytes),
            headers: BTreeMap::new(),
            timestamp: Utc::now(),
        };

        let partition_client = self
            .client
            .partition_client(topic, partition, UnknownTopicHandling::Retry)
            .await
            .map_err(|e| KafkaProducerError::Produce(e.to_string()))?;

        let offsets = partition_client
            .produce(vec![record], Compression::NoCompression)
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

/// Select a partition index by hashing the key.
///
/// Uses [`std::collections::hash_map::DefaultHasher`] (stable, no extra deps).
/// For single-partition topics this always returns 0.
fn select_partition(key: &str, num_partitions: i32) -> i32 {
    use std::hash::Hasher;

    if num_partitions <= 1 {
        return 0;
    }
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut hasher);
    let h = hasher.finish();
    (h % num_partitions as u64) as i32
}
