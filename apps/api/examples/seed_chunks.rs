//! Sprint 9 load-test seed script.
//!
//! Populates a running Chronik-Stream instance with N synthetic chunks
//! directly via the `published-pages` topic's upsert endpoint —
//! bypassing the API and the LLM embedding step.
//!
//! Vectors are generated with a deterministic PRNG so two runs produce
//! the same dataset: reproducible load tests. Each vector is
//! L2-normalized to match the geometry that HNSW expects from real
//! embedding models (OpenAI text-embedding-3-small, etc.).
//!
//! Usage:
//! ```bash
//! CHRONIK_SEARCH_URL=http://localhost:6092 \
//! SEED_COUNT=10000 \
//! SEED_BATCH_SIZE=200 \
//!   cargo run --release -p historiador_api --example seed_chunks
//! ```
//!
//! Env vars:
//! - `CHRONIK_SEARCH_URL`  — default `http://localhost:6092`
//! - `SEED_COUNT`          — number of chunks to write; default 10_000
//! - `SEED_BATCH_SIZE`     — upsert batch size; default 200
//! - `SEED_DIM`            — embedding dimension; default 1536
//! - `SEED_RNG_SEED`       — PRNG seed; default 42

use std::env;
use std::time::Instant;

use historiador_db::chronik::{ChronikClient, ChronikConfig};
use historiador_db::vector_store::ChunkEmbedding;
use rand::{rngs::StdRng, Rng, SeedableRng};
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

/// Generate a deterministic L2-normalized vector of the given dimension.
fn make_vector(rng: &mut StdRng, dim: usize) -> Vec<f32> {
    // Sample from N(0,1), then L2-normalize onto the unit sphere.
    let mut v: Vec<f32> = (0..dim).map(|_| rng.gen_range(-1.0_f32..1.0_f32)).collect();
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in &mut v {
            *x /= norm;
        }
    }
    v
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("seed_chunks=info,historiador_db=info")
        .init();

    let search_url = env_str("CHRONIK_SEARCH_URL", "http://localhost:6092");
    let total: usize = env_u64("SEED_COUNT", 10_000) as usize;
    let batch_size: usize = env_u64("SEED_BATCH_SIZE", 200) as usize;
    let dim: usize = env_u64("SEED_DIM", 1536) as usize;
    let seed: u64 = env_u64("SEED_RNG_SEED", 42);

    println!(
        "seeding {} chunks ({}-dim) into Chronik at {} in batches of {}",
        total, dim, search_url, batch_size
    );

    let client = ChronikClient::new(ChronikConfig {
        base_url: search_url.clone(),
        search_base_url: search_url.clone(),
    })?;

    // Sanity check before attempting bulk upserts.
    match client.search_health().await {
        Ok(true) => println!("chronik search endpoint healthy"),
        Ok(false) => anyhow::bail!("chronik search health check returned non-2xx"),
        Err(e) => anyhow::bail!("chronik search health check failed: {e}"),
    }

    // Pre-allocate page_version_ids. Each synthetic "page" owns
    // CHUNKS_PER_PAGE consecutive chunks. This keeps the metadata
    // enrichment join (apps/mcp/src/query.rs) exercising realistic
    // cardinality even though the page rows don't exist in Postgres.
    let page_count = total.div_ceil(CHUNKS_PER_PAGE);
    let mut rng = StdRng::seed_from_u64(seed);
    let page_version_ids: Vec<Uuid> = (0..page_count).map(|_| Uuid::new_v4()).collect();

    let start = Instant::now();
    let mut written = 0usize;
    let mut batch: Vec<ChunkEmbedding> = Vec::with_capacity(batch_size);

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

        batch.push(ChunkEmbedding {
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
            embedding: make_vector(&mut rng, dim),
        });

        if batch.len() >= batch_size {
            let chunks_to_send = std::mem::take(&mut batch);
            let refs = client.upsert_chunks(chunks_to_send).await?;
            written += refs.len();
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
        let refs = client.upsert_chunks(batch).await?;
        written += refs.len();
    }

    let total_elapsed = start.elapsed().as_secs_f64();
    println!(
        "done: {written} chunks written in {:.1}s ({:.0} chunks/s)",
        total_elapsed,
        written as f64 / total_elapsed
    );

    Ok(())
}
