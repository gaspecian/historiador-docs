//! Integration test: produce_event goes through Kafka, not REST.
//! Skipped if `CHRONIK_KAFKA_BROKER` is not set.

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
        kafka_broker: Some(broker),
    })
    .await
    .expect("client");

    // Use a throwaway topic.
    let topic = format!("test-events-{}", uuid::Uuid::new_v4());
    client
        .kafka_producer
        .as_ref()
        .expect("kafka producer should be present when broker is set")
        .ensure_topic(&topic, 1, None)
        .await
        .expect("ensure");

    client
        .produce_event(&topic, "k", &json!({"event": "ping"}))
        .await
        .expect("produce");
}
