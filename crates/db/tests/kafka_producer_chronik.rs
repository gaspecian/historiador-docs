//! Integration test against a running Chronik 2.4.1 on localhost:9092.
//! Skipped if `CHRONIK_KAFKA_BROKER` is not set.

use historiador_db::chronik::kafka_producer::KafkaProducer;
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
