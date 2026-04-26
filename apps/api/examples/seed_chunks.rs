//! Sprint 9 load-test seed script.
//!
//! Populates a running Chronik-Stream instance with N synthetic chunks
//! via the `published-pages` Kafka topic. Chronik embeds the content
//! server-side using its configured embedding model — this script does
//! not generate embeddings locally.
//!
//! Usage:
//! ```bash
//! CHRONIK_SQL_URL=http://localhost:6092 \
//! CHRONIK_KAFKA_BROKER=localhost:9092 \
//! SEED_COUNT=10000 \
//! SEED_BATCH_SIZE=200 \
//!   cargo run --release -p historiador_api --example seed_chunks
//! ```
//!
//! Env vars:
//! - `CHRONIK_SQL_URL`       — default `http://localhost:6092`
//! - `CHRONIK_KAFKA_BROKER`  — default `localhost:9092`
//! - `SEED_COUNT`            — number of chunks to write; default 10_000
//! - `SEED_BATCH_SIZE`       — produce batch size; default 200

use std::env;
use std::time::Instant;

use historiador_db::chronik::{ChronikClient, ChronikConfig};
use historiador_db::vector_store::{ChronikVectorStore, ChunkPayload, VectorStore};
use uuid::Uuid;

const LANGUAGES: &[&str] = &["en-US", "pt-BR", "es-ES"];
const COLLECTION_COUNT: u32 = 50;
const CHUNKS_PER_PAGE: usize = 8; // realistic: 6-10 chunks per page

fn env_u64(key: &str, default: u64) -> u64 {
    env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn env_str(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("seed_chunks=info,historiador_db=info")
        .init();

    let search_url = env_str("CHRONIK_SQL_URL", "http://localhost:6092");
    let kafka_broker = Some(env_str("CHRONIK_KAFKA_BROKER", "localhost:9092"));
    let total: usize = env_u64("SEED_COUNT", 10_000) as usize;
    let batch_size: usize = env_u64("SEED_BATCH_SIZE", 200) as usize;

    println!(
        "seeding {} chunks into Chronik at {} in batches of {}",
        total, search_url, batch_size
    );

    let client = ChronikClient::new(ChronikConfig {
        base_url: search_url.clone(),
        search_base_url: search_url.clone(),
        kafka_broker,
    })
    .await?;

    // Sanity check before bulk produce.
    match client.search_health().await {
        Ok(true) => println!("chronik search endpoint healthy"),
        Ok(false) => anyhow::bail!("chronik search health check returned non-2xx"),
        Err(e) => anyhow::bail!("chronik search health check failed: {e}"),
    }

    let store = ChronikVectorStore::new(client);

    // Pre-allocate page_version_ids. Each synthetic "page" owns
    // CHUNKS_PER_PAGE consecutive chunks.
    let page_count = total.div_ceil(CHUNKS_PER_PAGE);
    let page_version_ids: Vec<Uuid> = (0..page_count).map(|_| Uuid::new_v4()).collect();

    let start = Instant::now();
    let mut written = 0usize;
    let mut batch: Vec<ChunkPayload> = Vec::with_capacity(batch_size);

    for i in 0..total {
        let page_idx = i / CHUNKS_PER_PAGE;
        let section_index = (i % CHUNKS_PER_PAGE) as i32;
        let page_version_id = page_version_ids[page_idx].to_string();
        let collection = i as u32 % COLLECTION_COUNT;
        let language = LANGUAGES[(i / (CHUNKS_PER_PAGE * 10)) % LANGUAGES.len()];

        let content = format!(
            "Synthetic chunk {i} under collection {collection}. \
             This paragraph exists only to give the vector a passenger \
             and to keep Chronik's payload serialization honest."
        );

        batch.push(ChunkPayload {
            page_version_id,
            section_index,
            heading_path: vec![
                format!("Collection {collection}"),
                format!("Page {page_idx}"),
                format!("Section {section_index}"),
            ],
            content,
            language: language.to_string(),
            token_count: 64,
        });

        if batch.len() >= batch_size {
            let chunks_to_send = std::mem::take(&mut batch);
            let produced = store.produce_chunks(chunks_to_send).await?;
            written += produced.len();
            if written % (batch_size * 10) == 0 {
                let elapsed = start.elapsed().as_secs_f64();
                let rate = written as f64 / elapsed;
                println!(
                    "  wrote {written}/{total} ({:.0} chunks/s, {:.1}s elapsed)",
                    rate, elapsed
                );
            }
        }
    }

    if !batch.is_empty() {
        let produced = store.produce_chunks(batch).await?;
        written += produced.len();
    }

    let total_elapsed = start.elapsed().as_secs_f64();
    println!(
        "done: {written} chunks written in {:.1}s ({:.0} chunks/s)",
        total_elapsed,
        written as f64 / total_elapsed
    );

    Ok(())
}
