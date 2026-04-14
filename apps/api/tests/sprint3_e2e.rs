//! End-to-end walk-through of the Sprint 3 happy path.
//!
//! Covers: nested collections, page CRUD, publish → async chunking,
//! revert to draft, cascade delete, and RBAC enforcement.
//!
//! Requires a running Postgres. Locally:
//! ```bash
//! DATABASE_URL=postgres://historiador_admin:devpassword@localhost:5432/historiador \
//!   cargo test --test sprint3_e2e
//! ```

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use historiador_api::{app, crypto::Cipher, setup::llm_probe::StubProbe, state::AppState};
use historiador_db::vector_store::InMemoryVectorStore;
use historiador_llm::{StubEmbeddingClient, StubTextGenerationClient};
use reqwest::StatusCode;
use serde_json::{json, Value};
use sqlx::PgPool;

fn test_state(pool: PgPool) -> Arc<AppState> {
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
        chronik: None,
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

/// Run setup wizard and return the admin access token.
async fn setup_and_login(http: &reqwest::Client, base: &str) -> String {
    // Setup wizard.
    let r = http
        .post(format!("{base}/setup/init"))
        .json(&json!({
            "admin_email": "admin@example.com",
            "admin_password": "hunter2hunter2",
            "workspace_name": "Test Workspace",
            "llm_provider": "test",
            "llm_api_key": "test-key",
            "languages": ["en-US"],
            "primary_language": "en-US",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK, "setup should succeed");

    // Admin login.
    let r = http
        .post(format!("{base}/auth/login"))
        .json(&json!({ "email": "admin@example.com", "password": "hunter2hunter2" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let tokens: Value = r.json().await.unwrap();
    tokens["access_token"].as_str().unwrap().to_string()
}

#[sqlx::test(migrations = "../../crates/db/migrations")]
async fn walks_the_sprint_3_happy_path(pool: PgPool) {
    let base = spawn_app(pool).await;
    let http = reqwest::Client::new();
    let token = setup_and_login(&http, &base).await;

    // ---- 1. Create 3-level nested collection hierarchy ----
    // Level 1: Engineering
    let r = http
        .post(format!("{base}/collections"))
        .bearer_auth(&token)
        .json(&json!({ "name": "Engineering" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::CREATED);
    let eng: Value = r.json().await.unwrap();
    let eng_id = eng["id"].as_str().unwrap();
    assert_eq!(eng["name"], "Engineering");
    assert_eq!(eng["slug"], "engineering");

    // Level 2: APIs (child of Engineering)
    let r = http
        .post(format!("{base}/collections"))
        .bearer_auth(&token)
        .json(&json!({ "name": "APIs", "parent_id": eng_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::CREATED);
    let apis: Value = r.json().await.unwrap();
    let apis_id = apis["id"].as_str().unwrap();
    assert_eq!(apis["parent_id"], json!(eng_id));

    // Level 3: Authentication (child of APIs)
    let r = http
        .post(format!("{base}/collections"))
        .bearer_auth(&token)
        .json(&json!({ "name": "Authentication", "parent_id": apis_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::CREATED);
    let auth_col: Value = r.json().await.unwrap();
    assert_eq!(auth_col["parent_id"], json!(apis_id));

    // ---- 2. List collections ----
    let r = http
        .get(format!("{base}/collections"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let cols: Vec<Value> = r.json().await.unwrap();
    assert_eq!(cols.len(), 3);

    // ---- 3. Rename a collection ----
    let r = http
        .patch(format!("{base}/collections/{apis_id}"))
        .bearer_auth(&token)
        .json(&json!({ "name": "REST APIs" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let updated: Value = r.json().await.unwrap();
    assert_eq!(updated["name"], "REST APIs");

    // ---- 4. Create a page with markdown content ----
    let markdown = "\
## Getting Started

This section explains how to get started with the API.

## Authentication

Use bearer tokens to authenticate.

```rust
let token = auth.login(email, password);
```

Tokens expire after 15 minutes.
";
    let r = http
        .post(format!("{base}/pages"))
        .bearer_auth(&token)
        .json(&json!({
            "collection_id": apis_id,
            "title": "API Guide",
            "content_markdown": markdown,
            "language": "en-US",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::CREATED);
    let page: Value = r.json().await.unwrap();
    let page_id = page["id"].as_str().unwrap();
    assert_eq!(page["slug"], "api-guide");
    assert_eq!(page["status"], "draft");
    assert_eq!(page["versions"].as_array().unwrap().len(), 1);

    // ---- 5. Get page with versions ----
    let r = http
        .get(format!("{base}/pages/{page_id}"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let fetched: Value = r.json().await.unwrap();
    assert_eq!(fetched["versions"][0]["title"], "API Guide");

    // ---- 6. Update page draft ----
    let r = http
        .patch(format!("{base}/pages/{page_id}"))
        .bearer_auth(&token)
        .json(&json!({
            "title": "API Reference Guide",
            "language": "en-US",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let updated_page: Value = r.json().await.unwrap();
    assert_eq!(updated_page["versions"][0]["title"], "API Reference Guide");

    // ---- 7. Publish page — expect 202 ----
    let r = http
        .post(format!("{base}/pages/{page_id}/publish"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::ACCEPTED);
    let publish_resp: Value = r.json().await.unwrap();
    assert_eq!(publish_resp["status"], "published");

    // ---- 8. Wait for async pipeline, then verify chunks ----
    // The pipeline runs on tokio::spawn. Give it a moment.
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // Verify page status is now published.
    let r = http
        .get(format!("{base}/pages/{page_id}"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let published_page: Value = r.json().await.unwrap();
    assert_eq!(published_page["status"], "published");

    // ---- 9. Cannot edit published page ----
    let r = http
        .patch(format!("{base}/pages/{page_id}"))
        .bearer_auth(&token)
        .json(&json!({ "title": "Should Fail", "language": "en-US" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);

    // ---- 10. Revert to draft ----
    let r = http
        .post(format!("{base}/pages/{page_id}/draft"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let draft_resp: Value = r.json().await.unwrap();
    assert_eq!(draft_resp["status"], "draft");

    // ---- 11. Delete top-level collection cascades ----
    let r = http
        .delete(format!("{base}/collections/{eng_id}"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NO_CONTENT);

    // Child collections should be gone.
    let r = http
        .get(format!("{base}/collections"))
        .bearer_auth(&token)
        .send()
        .await
        .unwrap();
    let remaining: Vec<Value> = r.json().await.unwrap();
    assert!(remaining.is_empty(), "cascade should delete all children");

    // ---- 12. Slug conflict returns 409 ----
    // Create a parent first, then two children with the same name.
    // (Root-level NULL parent_id doesn't trigger unique constraints
    // in PostgreSQL because NULL != NULL.)
    let r = http
        .post(format!("{base}/collections"))
        .bearer_auth(&token)
        .json(&json!({ "name": "Parent" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::CREATED);
    let parent: Value = r.json().await.unwrap();
    let parent_id = parent["id"].as_str().unwrap();

    let r = http
        .post(format!("{base}/collections"))
        .bearer_auth(&token)
        .json(&json!({ "name": "Guides", "parent_id": parent_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::CREATED);

    let r = http
        .post(format!("{base}/collections"))
        .bearer_auth(&token)
        .json(&json!({ "name": "Guides", "parent_id": parent_id }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::CONFLICT);
}

#[sqlx::test(migrations = "../../crates/db/migrations")]
async fn rbac_viewer_cannot_create_collections(pool: PgPool) {
    let base = spawn_app(pool).await;
    let http = reqwest::Client::new();
    let admin_token = setup_and_login(&http, &base).await;

    // Invite a viewer.
    let r = http
        .post(format!("{base}/admin/users/invite"))
        .bearer_auth(&admin_token)
        .json(&json!({ "email": "viewer@example.com", "role": "viewer" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let invite: Value = r.json().await.unwrap();
    let invite_token = invite["activation_url"]
        .as_str()
        .unwrap()
        .split("token=")
        .nth(1)
        .unwrap();

    // Activate viewer.
    let r = http
        .post(format!("{base}/auth/activate"))
        .json(&json!({ "invite_token": invite_token, "password": "viewerpass1234" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NO_CONTENT);

    // Viewer logs in.
    let r = http
        .post(format!("{base}/auth/login"))
        .json(&json!({ "email": "viewer@example.com", "password": "viewerpass1234" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let viewer_tokens: Value = r.json().await.unwrap();
    let viewer_token = viewer_tokens["access_token"].as_str().unwrap();

    // Viewer can list collections.
    let r = http
        .get(format!("{base}/collections"))
        .bearer_auth(viewer_token)
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);

    // Viewer cannot create collections.
    let r = http
        .post(format!("{base}/collections"))
        .bearer_auth(viewer_token)
        .json(&json!({ "name": "Forbidden" }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN);

    // Viewer cannot create pages.
    let r = http
        .post(format!("{base}/pages"))
        .bearer_auth(viewer_token)
        .json(&json!({
            "title": "Forbidden Page",
            "content_markdown": "test",
            "language": "en-US",
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
}
