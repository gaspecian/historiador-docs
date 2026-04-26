//! Sprint 10 item #7 — seed Chronik with 10k synthetic chunks so the
//! MCP load test has something to query against.
//!
//! Chunks are produced to Chronik via Kafka; Chronik embeds server-side
//! (ADR-007). The `--dim` flag is accepted for backward-compatibility
//! but has no effect — embeddings are no longer generated client-side.
//!
//! Usage:
//!   # ensure Chronik is running and reachable:
//!   docker compose up -d chronik
//!
//!   CHRONIK_SQL_URL=http://localhost:6092 \
//!   CHRONIK_SEARCH_URL=http://localhost:6092 \
//!     cargo run --release -p historiador_api --bin load-test-seed \
//!       -- --chunks 10000
//!
//! Arguments:
//!   --chunks N     total chunk count to produce (default: 10000)
//!   --dim N        accepted but ignored (embeddings are server-side)
//!   --batch N      chunks per produce_chunks call (default: 100)
//!
//! Environment:
//!   CHRONIK_SQL_URL      required, e.g. http://localhost:6092
//!   CHRONIK_SEARCH_URL   optional, defaults to CHRONIK_SQL_URL
//!   CHRONIK_KAFKA_BROKER optional, defaults to localhost:9092

use std::time::Instant;

use anyhow::Context;

use historiador_db::chronik::{ChronikClient, ChronikConfig};
use historiador_db::vector_store::{ChronikVectorStore, ChunkPayload, VectorStore};

struct Args {
    chunks: usize,
    batch: usize,
}

fn parse_args() -> anyhow::Result<Args> {
    let mut chunks: usize = 10_000;
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
            "--dim" => {
                // Accepted for backward-compatibility; ignored — embeddings
                // are generated server-side by Chronik.
                let _ = parse_next(&mut it, "--dim")?;
            }
            "--batch" => batch = parse_next(&mut it, "--batch")?,
            "-h" | "--help" => {
                eprintln!(include_str!("load_test_seed_usage.txt"));
                std::process::exit(0);
            }
            other => anyhow::bail!("unknown flag: {other}"),
        }
    }

    if chunks == 0 || batch == 0 {
        anyhow::bail!("--chunks and --batch must both be > 0");
    }
    Ok(Args { chunks, batch })
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    let args = parse_args()?;

    let chronik_url = std::env::var("CHRONIK_SQL_URL")
        .context("CHRONIK_SQL_URL is required (e.g. http://localhost:6092)")?;
    let search_url = std::env::var("CHRONIK_SEARCH_URL").unwrap_or_else(|_| chronik_url.clone());

    eprintln!(
        "seeding {chunks} chunks (batch={batch}) into Chronik @ {url}",
        chunks = args.chunks,
        batch = args.batch,
        url = chronik_url
    );

    let kafka_broker = Some(
        std::env::var("CHRONIK_KAFKA_BROKER").unwrap_or_else(|_| "localhost:9092".to_string()),
    );
    let client = ChronikClient::new(ChronikConfig {
        base_url: chronik_url,
        search_base_url: search_url,
        kafka_broker,
    })
    .await?;
    let store = ChronikVectorStore::new(client);

    // Fail fast if Chronik is not reachable — the load test would be
    // meaningless otherwise.
    if !store.health().await? {
        anyhow::bail!("Chronik health probe returned false — is the container running?");
    }

    let start = Instant::now();
    let mut remaining = args.chunks;

    while remaining > 0 {
        let this_batch = remaining.min(args.batch);
        let payloads: Vec<ChunkPayload> = (0..this_batch)
            .map(|i| {
                let global_idx = args.chunks - remaining + i;
                synthetic_chunk(global_idx)
            })
            .collect();

        store
            .produce_chunks(payloads)
            .await
            .with_context(|| format!("produce failed at offset {}", args.chunks - remaining))?;

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

fn synthetic_chunk(idx: usize) -> ChunkPayload {
    let lang_variants = ["en", "pt-BR", "es", "fr"];
    let language = lang_variants[idx % lang_variants.len()].to_string();

    ChunkPayload {
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
    }
}
