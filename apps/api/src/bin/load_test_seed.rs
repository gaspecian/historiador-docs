//! Sprint 10 item #7 — seed Chronik with 10k synthetic chunks so the
//! MCP load test has something to query against.
//!
//! The chunks are deliberately synthetic (randomized content +
//! deterministic embeddings) so the load test exercises the vector
//! store's indexing + search path, not the embedding provider. This
//! keeps the test hermetic: it does not call OpenAI or Ollama, and
//! the p50/p95 numbers reflect only the retrieval substrate.
//!
//! Usage:
//!   # ensure Chronik is running and reachable:
//!   docker compose up -d chronik
//!
//!   CHRONIK_SQL_URL=http://localhost:6092 \
//!   CHRONIK_SEARCH_URL=http://localhost:6092 \
//!     cargo run --release -p historiador_api --bin load-test-seed \
//!       -- --chunks 10000 --dim 1536
//!
//! Arguments:
//!   --chunks N     total chunk count to upsert (default: 10000)
//!   --dim N        embedding dimension (default: 1536, matches text-
//!                  embedding-3-small and the in-memory stub default)
//!   --batch N      chunks per upsert_chunks call (default: 100)
//!
//! Environment:
//!   CHRONIK_SQL_URL     required, e.g. http://localhost:6092
//!   CHRONIK_SEARCH_URL  optional, defaults to CHRONIK_SQL_URL

use std::time::Instant;

use anyhow::Context;

use historiador_db::chronik::{ChronikClient, ChronikConfig};
use historiador_db::vector_store::{ChronikVectorStore, ChunkEmbedding, VectorStore};

struct Args {
    chunks: usize,
    dim: usize,
    batch: usize,
}

fn parse_args() -> anyhow::Result<Args> {
    let mut chunks: usize = 10_000;
    let mut dim: usize = 1536;
    let mut batch: usize = 100;

    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let parse_next =
            |it: &mut dyn Iterator<Item = String>, flag: &str| -> anyhow::Result<usize> {
                let raw = it.next().with_context(|| format!("{flag} needs a value"))?;
                raw.parse()
                    .with_context(|| format!("{flag} must be a positive integer"))
            };
        match arg.as_str() {
            "--chunks" => chunks = parse_next(&mut it, "--chunks")?,
            "--dim" => dim = parse_next(&mut it, "--dim")?,
            "--batch" => batch = parse_next(&mut it, "--batch")?,
            "-h" | "--help" => {
                eprintln!(include_str!("load_test_seed_usage.txt"));
                std::process::exit(0);
            }
            other => anyhow::bail!("unknown flag: {other}"),
        }
    }

    if chunks == 0 || dim == 0 || batch == 0 {
        anyhow::bail!("--chunks, --dim, --batch must all be > 0");
    }
    Ok(Args { chunks, dim, batch })
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    let args = parse_args()?;

    let chronik_url = std::env::var("CHRONIK_SQL_URL")
        .context("CHRONIK_SQL_URL is required (e.g. http://localhost:6092)")?;
    let search_url = std::env::var("CHRONIK_SEARCH_URL").unwrap_or_else(|_| chronik_url.clone());

    eprintln!(
        "seeding {chunks} chunks (dim={dim}, batch={batch}) into Chronik @ {url}",
        chunks = args.chunks,
        dim = args.dim,
        batch = args.batch,
        url = chronik_url
    );

    let client = ChronikClient::new(ChronikConfig {
        base_url: chronik_url,
        search_base_url: search_url,
    })?;
    let store = ChronikVectorStore::new(client);

    // Fail fast if Chronik is not reachable — the load test would be
    // meaningless otherwise.
    if !store.health().await? {
        anyhow::bail!("Chronik health probe returned false — is the container running?");
    }

    let start = Instant::now();
    let mut remaining = args.chunks;
    let mut rng_state: u64 = 0xcafe_babe_dead_beef;

    while remaining > 0 {
        let this_batch = remaining.min(args.batch);
        let chunks: Vec<ChunkEmbedding> = (0..this_batch)
            .map(|i| {
                let global_idx = args.chunks - remaining + i;
                synthetic_chunk(global_idx, args.dim, &mut rng_state)
            })
            .collect();

        store
            .upsert_chunks(chunks)
            .await
            .with_context(|| format!("upsert failed at offset {}", args.chunks - remaining))?;

        remaining -= this_batch;
        eprint!("\rseeded {}/{}", args.chunks - remaining, args.chunks);
    }
    eprintln!();

    let elapsed = start.elapsed();
    println!(
        "done: {chunks} chunks in {secs:.1}s ({rate:.0} chunks/s)",
        chunks = args.chunks,
        secs = elapsed.as_secs_f64(),
        rate = args.chunks as f64 / elapsed.as_secs_f64()
    );
    Ok(())
}

fn synthetic_chunk(idx: usize, dim: usize, rng_state: &mut u64) -> ChunkEmbedding {
    // xorshift64 is deterministic + cheap; we don't need
    // cryptographic quality for synthetic benchmark data.
    let embedding: Vec<f32> = (0..dim)
        .map(|_| (xorshift64(rng_state) as f32) / (u64::MAX as f32) * 2.0 - 1.0)
        .collect();

    let lang_variants = ["en", "pt-BR", "es", "fr"];
    let language = lang_variants[idx % lang_variants.len()].to_string();

    ChunkEmbedding {
        // Synthetic UUID-ish string so de-dupe-by-page-version still
        // distributes across pages. 500 distinct page_versions × ~20
        // chunks each at the default N=10k.
        page_version_id: format!("00000000-0000-4000-8000-{:012x}", idx / 20),
        section_index: (idx % 20) as i32,
        heading_path: vec![
            format!("Chapter {}", idx / 100),
            format!("Section {}", (idx / 10) % 10),
        ],
        content: format!(
            "Synthetic chunk {idx}. The quick brown fox jumps over the lazy dog. \
             Lorem ipsum dolor sit amet, consectetur adipiscing elit. {idx} \
             documentation content content content."
        ),
        language,
        token_count: 32,
        embedding,
    }
}

fn xorshift64(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}
