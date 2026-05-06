use historiador_db::chronik::kafka_producer::published_pages_topic_config;
use historiador_db::chronik::{ChronikClient, ChronikConfig};
use historiador_db::vector_store::{ChronikVectorStore, ChunkPayload, SearchFilters, VectorStore};

#[tokio::test]
async fn produce_chunks_returns_partition_offset() {
    // This part of the test does NOT require Chronik to embed; it
    // just confirms produce returns valid (partition, offset) tuples.
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

    // All topics are provisioned with 1 partition at boot (Chronik 2.4.1
    // single-broker constraint). ensure_topic is idempotent — if the topic
    // already exists from a previous run this is a no-op.
    let producer = client.kafka_producer.as_ref().expect("producer present");
    producer
        .ensure_topic(
            historiador_db::chronik::producer::topics::PUBLISHED_PAGES,
            1,
            Some(published_pages_topic_config()),
        )
        .await
        .expect("ensure published-pages topic");

    let store = ChronikVectorStore::new(client);
    // Unique page_version_id per run — provides per-run isolation without
    // needing to delete/recreate the topic.
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
}

#[tokio::test]
async fn produce_then_search_recovers_partition_offset() {
    // Full e2e test — requires Chronik to embed via OpenAI. Skipped
    // unless EMBEDDING_API_KEY is set on the host AND Chronik
    // container has been restarted with that key visible (Task 11
    // plumbs the docker-compose env). Without it Chronik will
    // produce ok but never index, and this test will time out.
    let Some(broker) = std::env::var("CHRONIK_KAFKA_BROKER").ok() else {
        eprintln!("skipping: CHRONIK_KAFKA_BROKER not set");
        return;
    };
    if std::env::var("EMBEDDING_API_KEY").is_err() {
        eprintln!("skipping: EMBEDDING_API_KEY not set (Chronik can't embed)");
        return;
    }

    let client = ChronikClient::new(ChronikConfig {
        base_url: "http://localhost:6092".into(),
        search_base_url: "http://localhost:6092".into(),
        kafka_broker: Some(broker),
    })
    .await
    .expect("client");

    // All topics are provisioned with 1 partition at boot. ensure_topic is
    // idempotent; isolation is provided by the unique pv_id below.
    let producer = client.kafka_producer.as_ref().expect("producer present");
    producer
        .ensure_topic(
            historiador_db::chronik::producer::topics::PUBLISHED_PAGES,
            1,
            Some(published_pages_topic_config()),
        )
        .await
        .expect("ensure published-pages topic");

    let store = ChronikVectorStore::new(client);
    // Unique page_version_id per run — the search filter on pv_id ensures
    // results from this run are not confused with those of concurrent runs.
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

    // Allow Chronik's background indexer to catch up.
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

    assert!(
        !hits.is_empty(),
        "expected at least one chunk back from Chronik within 60s"
    );
    let first = &hits[0];
    assert_eq!(first.partition, produced[0].partition);
    assert_eq!(first.offset, produced[0].offset);
}
