//! End-to-end: publish a page through the API, then query it through
//! the MCP server. Requires real Chronik on :9092 and Postgres on
//! :5432 (i.e. `docker compose up -d`).
//!
//! Run with both env vars set to exercise the full pipeline:
//! ```bash
//! DATABASE_URL=postgres://historiador_admin:devpassword@localhost:5432/historiador \
//! CHRONIK_KAFKA_BROKER=localhost:9092 \
//! EMBEDDING_API_KEY=sk-... \
//! cargo test -p historiador_mcp --test mcp_query_e2e -- --nocapture
//! ```
//!
//! Skipped automatically when CHRONIK_KAFKA_BROKER or EMBEDDING_API_KEY is
//! absent — the test reports 0 failures in that case.
//!
//! Note: we use `#[tokio::test]` (not `#[sqlx::test]`) so that the env-var
//! skip guards run before any database connection is attempted. The pool is
//! created manually inside the test body after the guards pass.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use historiador_api::{
    app,
    infrastructure::backfill::{shared_disabled, BackfillService, BackfillState},
    infrastructure::crypto::raw::Cipher,
    infrastructure::llm::probe::{LlmProbe, StubProbe},
    infrastructure::prompts::LoadedPrompt,
    infrastructure::telemetry::editor::EditorMetrics,
    presentation::{BuildDeps, UseCases},
    state::AppState,
};
use historiador_db::{
    chronik::{ChronikClient, ChronikConfig},
    vector_store::{ChronikVectorStore, VectorStore},
};
use historiador_llm::{StubTextGenerationClient, TextGenerationClient};
use historiador_mcp::{
    application::SearchChunksUseCase, build_router as mcp_build_router,
    infrastructure::PostgresChunkMetadataReader, state::McpState,
};
use sqlx::PgPool;

const POLL_TIMEOUT: Duration = Duration::from_secs(120);
const POLL_INTERVAL: Duration = Duration::from_secs(2);

/// Fixed bearer token for MCP auth during the test. Both the McpState
/// (which stores its SHA-256 hash) and the HTTP client use this value.
const MCP_TOKEN: &str = "test-mcp-bearer-token-e2e";

// ---------------------------------------------------------------------------
// Helpers — mirrored from apps/api/tests/sprint3_e2e.rs
// ---------------------------------------------------------------------------

fn api_test_state(pool: PgPool, chronik: ChronikClient) -> Arc<AppState> {
    let enc_key = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
    let cipher = Cipher::from_base64(enc_key).unwrap();
    let jwt_secret: Vec<u8> = b"test-secret-at-least-32-bytes-long-xxxx".to_vec();
    let llm_probe: Arc<dyn LlmProbe> = Arc::new(StubProbe);
    // Use ChronikVectorStore so the publish pipeline produces real chunks.
    let vector_store: Arc<dyn VectorStore> = Arc::new(ChronikVectorStore::new(chronik.clone()));
    let text_gen: Arc<dyn TextGenerationClient> = Arc::new(StubTextGenerationClient);

    let use_cases = Arc::new(UseCases::build(BuildDeps {
        pool: pool.clone(),
        cipher: cipher.clone(),
        jwt_secret: jwt_secret.clone(),
        llm_probe: llm_probe.clone(),
        vector_store: vector_store.clone(),
        text_generation_client: text_gen.clone(),
        chronik: Some(chronik),
    }));

    Arc::new(AppState {
        pool,
        git_sha: "e2e-test".into(),
        jwt_secret,
        cipher,
        public_base_url: "http://localhost:3000".into(),
        setup_complete: AtomicBool::new(false),
        llm_probe,
        vector_store,
        text_generation_client: text_gen,
        // The ChronikClient was already passed into UseCases above; keep
        // AppState::chronik as None to avoid a double-producer path.
        chronik: None,
        use_cases,
        editor_v2_enabled: false,
        agent_prompt: Arc::new(LoadedPrompt::for_test()),
        editor_metrics: Arc::new(EditorMetrics::new()),
        backfill_state: shared_disabled(),
    })
}

/// Spin up the API on a random TCP port and return its base URL.
async fn spawn_api(pool: PgPool, chronik: ChronikClient) -> String {
    let state = api_test_state(pool, chronik);
    let router = app::build_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    format!("http://{addr}")
}

/// Spin up the MCP server on a random TCP port and return its base URL.
async fn spawn_mcp(pool: PgPool, chronik: ChronikClient) -> String {
    let token_hash: [u8; 32] = Sha256::digest(MCP_TOKEN.as_bytes()).into();
    let vector_store: Arc<dyn VectorStore> = Arc::new(ChronikVectorStore::new(chronik));
    let metadata_reader = Arc::new(PostgresChunkMetadataReader::new(pool));
    let search_chunks = Arc::new(SearchChunksUseCase::new(vector_store, metadata_reader));

    let state = Arc::new(McpState {
        search_chunks,
        bearer_token_hash: token_hash,
        // Telemetry log is fire-and-forget; a dummy URL is fine here.
        internal_api_url: "http://127.0.0.1:1".into(),
        workspace_id: uuid::Uuid::nil(),
    });

    let router = mcp_build_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    format!("http://{addr}")
}

/// Run the API setup wizard and return the admin JWT.
async fn setup_and_login(http: &reqwest::Client, api_base: &str) -> String {
    let r = http
        .post(format!("{api_base}/setup/init"))
        .json(&json!({
            "admin_email": "admin@e2e.test",
            "admin_password": "hunter2hunter2",
            "workspace_name": "E2E Workspace",
            "llm_provider": "test",
            "llm_api_key": "test-key",
            "languages": ["en-US"],
            "primary_language": "en-US",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        r.status(),
        200,
        "setup/init failed: {}",
        r.text().await.unwrap()
    );

    let r = http
        .post(format!("{api_base}/auth/login"))
        .json(&json!({ "email": "admin@e2e.test", "password": "hunter2hunter2" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200, "login failed");
    let tokens: Value = r.json().await.unwrap();
    tokens["access_token"].as_str().unwrap().to_string()
}

// ---------------------------------------------------------------------------
// The test
// ---------------------------------------------------------------------------

/// Publishes a page through the API, then polls the MCP `query` tool
/// until the chunk appears (or the 120 s deadline elapses).
///
/// Skipped unless both `CHRONIK_KAFKA_BROKER` and `EMBEDDING_API_KEY`
/// are present in the environment. Also requires `DATABASE_URL` and a
/// running Postgres instance.
#[tokio::test]
async fn publish_then_mcp_query_returns_the_chunk() {
    // --- skip guards (must run before any network I/O) ---
    let kafka_broker = match std::env::var("CHRONIK_KAFKA_BROKER") {
        Ok(v) => v,
        Err(_) => {
            eprintln!("skipping: CHRONIK_KAFKA_BROKER not set");
            return;
        }
    };
    if std::env::var("EMBEDDING_API_KEY").is_err() {
        eprintln!("skipping: EMBEDDING_API_KEY required for Chronik to embed");
        return;
    }
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(v) => v,
        Err(_) => {
            eprintln!("skipping: DATABASE_URL not set");
            return;
        }
    };

    // --- real infrastructure ---
    let pool: PgPool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("connect to postgres");

    // Run migrations so the schema is up to date on whatever DB we got.
    historiador_db::run_migrations(&pool)
        .await
        .expect("run migrations");

    let chronik_sql_url =
        std::env::var("CHRONIK_SQL_URL").unwrap_or_else(|_| "http://localhost:6092".to_string());
    let chronik_search_url =
        std::env::var("CHRONIK_SEARCH_URL").unwrap_or_else(|_| chronik_sql_url.clone());

    let chronik = ChronikClient::new(ChronikConfig {
        base_url: chronik_sql_url,
        search_base_url: chronik_search_url,
        kafka_broker: Some(kafka_broker),
    })
    .await
    .expect("connect to Chronik");

    // --- boot both servers in-process ---
    let api_base = spawn_api(pool.clone(), chronik.clone()).await;
    let mcp_base = spawn_mcp(pool.clone(), chronik.clone()).await;

    let http = reqwest::Client::new();
    let token = setup_and_login(&http, &api_base).await;

    // --- create collection ---
    let r = http
        .post(format!("{api_base}/collections"))
        .bearer_auth(&token)
        .json(&json!({ "name": "E2E Docs" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 201, "create collection failed");
    let col: Value = r.json().await.unwrap();
    let col_id = col["id"].as_str().unwrap();

    // --- create page with a distinctive phrase ---
    // The phrase is designed to be semantically unique so the vector
    // search reliably returns it even with a small index.
    let unique_phrase = "the-quick-brown-fox-e2e-1a2b3c";
    let markdown = format!(
        "## E2E Test Section\n\n\
         This page contains the unique phrase: {unique_phrase}.\n\
         It also mentions dogs jumping over lazy animals.\n"
    );
    let r = http
        .post(format!("{api_base}/pages"))
        .bearer_auth(&token)
        .json(&json!({
            "collection_id": col_id,
            "title": "E2E MCP Query Page",
            "content_markdown": markdown,
            "language": "en-US",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(
        r.status(),
        201,
        "create page failed: {}",
        r.text().await.unwrap()
    );
    let page: Value = r.json().await.unwrap();
    let page_id = page["id"].as_str().unwrap();

    // --- publish (returns 202; chunk pipeline is async) ---
    let r = http
        .post(format!("{api_base}/pages/{page_id}/publish"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(
        r.status(),
        202,
        "publish failed: {}",
        r.text().await.unwrap()
    );

    // --- poll MCP query until the chunk appears ---
    // Chronik embeds server-side; the round-trip can take 30–60 s.
    let mcp_payload = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "query",
            "arguments": {
                "query": unique_phrase,
                "top_k": 5
            }
        }
    });

    let deadline = Instant::now() + POLL_TIMEOUT;
    let mut found = false;

    while Instant::now() < deadline {
        let r = http
            .post(format!("{mcp_base}/mcp"))
            .bearer_auth(MCP_TOKEN)
            .json(&mcp_payload)
            .send()
            .await
            .unwrap();

        if r.status() == 200 {
            let resp: Value = r.json().await.unwrap();
            // tools/call success shape:
            //   result.content[0].text = JSON string of QueryResponse
            if let Some(text) = resp["result"]["content"][0]["text"].as_str() {
                if let Ok(query_resp) = serde_json::from_str::<Value>(text) {
                    let chunks = query_resp["chunks"].as_array().cloned().unwrap_or_default();
                    if !chunks.is_empty() {
                        let hit = chunks.iter().any(|c| {
                            c["page_title"]
                                .as_str()
                                .unwrap_or("")
                                .contains("E2E MCP Query Page")
                                || c["content"].as_str().unwrap_or("").contains(unique_phrase)
                        });
                        if hit {
                            found = true;
                            break;
                        }
                    }
                }
            }
        }

        tokio::time::sleep(POLL_INTERVAL).await;
    }

    assert!(
        found,
        "MCP query did not return the published chunk within {}s",
        POLL_TIMEOUT.as_secs()
    );
}

/// Cold backfill end-to-end: seeds a published page version directly
/// in Postgres (skipping the publish path so Chronik stays empty for
/// it), runs `BackfillService::run()`, then asserts the page version
/// shows up in Chronik's `published-pages` topic.
///
/// Skipped under the same env-var guards as the main MCP test.
#[tokio::test]
async fn backfill_publishes_missing_page_versions_to_chronik() {
    // --- skip guards ---
    let kafka_broker = match std::env::var("CHRONIK_KAFKA_BROKER") {
        Ok(v) => v,
        Err(_) => {
            eprintln!("skipping: CHRONIK_KAFKA_BROKER not set");
            return;
        }
    };
    if std::env::var("EMBEDDING_API_KEY").is_err() {
        eprintln!("skipping: EMBEDDING_API_KEY required for Chronik to embed");
        return;
    }
    let database_url = match std::env::var("DATABASE_URL") {
        Ok(v) => v,
        Err(_) => {
            eprintln!("skipping: DATABASE_URL not set");
            return;
        }
    };

    let pool: PgPool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("connect to postgres");
    historiador_db::run_migrations(&pool)
        .await
        .expect("run migrations");

    let chronik_sql_url =
        std::env::var("CHRONIK_SQL_URL").unwrap_or_else(|_| "http://localhost:6092".to_string());
    let chronik_search_url =
        std::env::var("CHRONIK_SEARCH_URL").unwrap_or_else(|_| chronik_sql_url.clone());

    let chronik = ChronikClient::new(ChronikConfig {
        base_url: chronik_sql_url,
        search_base_url: chronik_search_url,
        kafka_broker: Some(kafka_broker),
    })
    .await
    .expect("connect to Chronik");

    // Make sure the published-pages topic exists so the SQL probe has
    // something to query (Chronik's DataFusion view fails on missing
    // topics). Idempotent — tolerates already-exists.
    if let Some(ref kafka) = chronik.kafka_producer {
        let _ = kafka
            .ensure_topic(
                historiador_db::chronik::producer::topics::PUBLISHED_PAGES,
                1,
                Some(historiador_db::chronik::kafka_producer::published_pages_topic_config()),
            )
            .await;
    }

    // --- seed workspace + page + published page_version via raw SQL ---
    // Random suffix keeps this independent of the other test in the file.
    let workspace_name = format!("Backfill Test {}", uuid::Uuid::new_v4());
    let workspace_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO workspaces \
           (name, languages, primary_language, generation_model) \
         VALUES ($1, ARRAY['en-US']::TEXT[], 'en-US', 'stub') \
         RETURNING id",
    )
    .bind(&workspace_name)
    .fetch_one(&pool)
    .await
    .expect("insert workspace");

    let page_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO pages (workspace_id, slug, status) \
         VALUES ($1, $2, 'published') RETURNING id",
    )
    .bind(workspace_id)
    .bind(format!("backfill-{}", uuid::Uuid::new_v4()))
    .fetch_one(&pool)
    .await
    .expect("insert page");

    let unique_marker = format!("backfill-marker-{}", uuid::Uuid::new_v4().simple());
    let markdown = format!("## Backfill Test\n\nThis page contains: {unique_marker}\n");
    let page_version_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO page_versions \
           (page_id, language, title, content_markdown, status) \
         VALUES ($1, 'en-US', 'Backfill Test', $2, 'published') \
         RETURNING id",
    )
    .bind(page_id)
    .bind(&markdown)
    .fetch_one(&pool)
    .await
    .expect("insert page_version");

    // Sanity: Chronik does not yet contain this page_version_id.
    let pre = chronik
        .list_synced_page_version_ids()
        .await
        .expect("chronik probe (pre)");
    assert!(
        !pre.iter().any(|s| s == &page_version_id.to_string()),
        "page_version {page_version_id} unexpectedly already present in Chronik"
    );

    // --- run the backfill service ---
    let vector_store: Arc<dyn VectorStore> = Arc::new(ChronikVectorStore::new(chronik.clone()));
    let state = shared_disabled();
    let svc = BackfillService::new(
        pool.clone(),
        chronik.clone(),
        vector_store.clone(),
        state.clone(),
    );
    svc.run().await;

    // Assert state reflects success.
    let snapshot = state.read().expect("state lock").clone();
    match snapshot {
        BackfillState::Completed { synced, failed } => {
            assert!(
                synced >= 1,
                "expected at least 1 synced page_version, got {synced}"
            );
            assert_eq!(failed, 0, "expected 0 failures, got {failed}");
        }
        other => panic!("expected Completed, got {other:?}"),
    }

    // Verify the chunk pipeline actually produced to Chronik by
    // checking the Postgres `chunks` table: rows here only get a
    // non-null (chronik_partition, chronik_offset) after the Kafka
    // produce returns success. This is the same signal the normal
    // publish path relies on. We avoid re-probing Chronik via SQL
    // because its DataFusion catalog lags the Kafka log by an
    // unbounded amount on this build.
    let chunk_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM chunks \
         WHERE page_version_id = $1 \
           AND chronik_partition IS NOT NULL \
           AND chronik_offset IS NOT NULL",
    )
    .bind(page_version_id)
    .fetch_one(&pool)
    .await
    .expect("count chunks");
    assert!(
        chunk_count >= 1,
        "expected at least 1 chunk row with Chronik offsets for {page_version_id}, got {chunk_count}"
    );
}
