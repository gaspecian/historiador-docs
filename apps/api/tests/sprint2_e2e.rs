//! End-to-end walk-through of the Sprint 2 happy path.
//!
//! Uses `sqlx::test` to get a fresh, migrated Postgres database per
//! test. The sqlx test macro reads `DATABASE_URL` at runtime — it
//! must point at a reachable Postgres server. In CI this is a
//! `postgres` service container; locally, the easiest source is
//! `docker compose up postgres` from the repo root, then:
//!
//! ```bash
//! DATABASE_URL=postgres://historiador_admin:devpassword@localhost:5432/historiador \
//!   cargo test --test sprint2_e2e
//! ```
//!
//! The test boots the full Axum app via `build_router`, binds to an
//! ephemeral port, and drives the entire sprint walk-through with
//! `reqwest` — same surface area as `curl` against a running stack.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use historiador_api::{app, crypto::Cipher, setup::llm_probe::StubProbe, state::AppState};
use historiador_db::vector_store::InMemoryVectorStore;
use historiador_llm::{StubEmbeddingClient, StubTextGenerationClient};
use reqwest::StatusCode;
use serde_json::{json, Value};
use sqlx::PgPool;

fn test_state(pool: PgPool) -> Arc<AppState> {
    // 32 zero bytes, base64 — test fixture only.
    let enc_key = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
    Arc::new(AppState {
        pool,
        git_sha: "test".into(),
        jwt_secret: b"test-secret-at-least-32-bytes-long-xxxx".to_vec(),
        cipher: Cipher::from_base64(enc_key).unwrap(),
        public_base_url: "http://localhost:3000".into(),
        setup_complete: AtomicBool::new(false),
        llm_probe: Arc::new(StubProbe),
        vector_store: Arc::new(InMemoryVectorStore::new()),
        embedding_client: Arc::new(StubEmbeddingClient::default()),
        text_generation_client: Arc::new(StubTextGenerationClient),
    })
}

async fn spawn_app(pool: PgPool) -> String {
    let state = test_state(pool);
    let router = app::build_router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    format!("http://{addr}")
}

#[sqlx::test(migrations = "../../crates/db/migrations")]
async fn walks_the_sprint_2_happy_path(pool: PgPool) {
    let base = spawn_app(pool).await;
    let http = reqwest::Client::new();

    // 1. Gate blocks everything except /health and /setup/init.
    let r = http.get(format!("{base}/pages/")).send().await.unwrap();
    assert_eq!(r.status(), StatusCode::LOCKED, "gate should return 423");

    let r = http.get(format!("{base}/health")).send().await.unwrap();
    assert_eq!(r.status(), StatusCode::OK);

    // 2. Setup wizard succeeds with a valid payload (StubProbe accepts).
    let setup_body = json!({
        "admin_email": "admin@example.com",
        "admin_password": "hunter2hunter2",
        "workspace_name": "Test Workspace",
        "llm_provider": "openai",
        "llm_api_key": "sk-test-stub",
        "languages": ["pt-BR", "en-US"],
        "primary_language": "pt-BR",
    });
    let r = http
        .post(format!("{base}/setup/init"))
        .json(&setup_body)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK, "setup should succeed");
    let body: Value = r.json().await.unwrap();
    assert_eq!(body["setup_complete"], json!(true));

    // 3. Running setup again fails with 409.
    let r = http
        .post(format!("{base}/setup/init"))
        .json(&setup_body)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::CONFLICT);

    // 4. Admin can log in.
    let r = http
        .post(format!("{base}/auth/login"))
        .json(&json!({ "email": "admin@example.com", "password": "hunter2hunter2" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let tokens: Value = r.json().await.unwrap();
    let admin_access = tokens["access_token"].as_str().unwrap().to_string();
    let admin_refresh = tokens["refresh_token"].as_str().unwrap().to_string();
    assert!(!admin_access.is_empty());

    // 5. Wrong password is rejected.
    let r = http
        .post(format!("{base}/auth/login"))
        .json(&json!({ "email": "admin@example.com", "password": "wrong" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);

    // 6. Admin invites an author. Capture the activation URL.
    let r = http
        .post(format!("{base}/admin/users/invite"))
        .bearer_auth(&admin_access)
        .json(&json!({ "email": "author@example.com", "role": "author" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let invite_body: Value = r.json().await.unwrap();
    let activation_url = invite_body["activation_url"].as_str().unwrap().to_string();
    let invite_token = activation_url.split("token=").nth(1).unwrap().to_string();

    // 7. Author activates with the token + sets their password.
    let r = http
        .post(format!("{base}/auth/activate"))
        .json(&json!({ "invite_token": invite_token, "password": "authorpass12345" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NO_CONTENT);

    // 8. Author can now log in.
    let r = http
        .post(format!("{base}/auth/login"))
        .json(&json!({ "email": "author@example.com", "password": "authorpass12345" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let author_tokens: Value = r.json().await.unwrap();
    let author_access = author_tokens["access_token"].as_str().unwrap().to_string();

    // 9. RBAC: author cannot invite users.
    let r = http
        .post(format!("{base}/admin/users/invite"))
        .bearer_auth(&author_access)
        .json(&json!({ "email": "viewer@example.com", "role": "viewer" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN);

    // 10. Unauthenticated invite is 401.
    let r = http
        .post(format!("{base}/admin/users/invite"))
        .json(&json!({ "email": "x@example.com", "role": "viewer" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);

    // 11. Admin refresh rotates the token.
    let r = http
        .post(format!("{base}/auth/refresh"))
        .json(&json!({ "refresh_token": admin_refresh }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let rotated: Value = r.json().await.unwrap();
    let new_refresh = rotated["refresh_token"].as_str().unwrap().to_string();
    assert_ne!(new_refresh, admin_refresh);

    // 12. Old refresh token is now dead.
    let r = http
        .post(format!("{base}/auth/refresh"))
        .json(&json!({ "refresh_token": admin_refresh }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);

    // 13. Logout invalidates the refresh token.
    let r = http
        .post(format!("{base}/auth/logout"))
        .json(&json!({ "refresh_token": new_refresh }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NO_CONTENT);

    let r = http
        .post(format!("{base}/auth/refresh"))
        .json(&json!({ "refresh_token": new_refresh }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
}
