//! Route group nests. Each sub-router is mounted into the top-level
//! app tree by [`crate::app::build_router`]. Handlers are imported
//! under short aliases so the route table stays scannable.

use axum::{
    routing::{get, patch, post},
    Router,
};
use std::sync::Arc;

use crate::presentation::handler::admin::{
    analytics as admin_analytics, users as admin_users, workspace as admin_workspace,
};
use crate::presentation::handler::{auth, collections, editor, editor_ws, export, pages, setup};
use crate::state::AppState;

pub fn auth_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/login", post(auth::login))
        .route("/refresh", post(auth::refresh))
        .route("/logout", post(auth::logout))
        .route("/activate", post(auth::activate))
}

pub fn setup_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/init", post(setup::init))
        .route("/probe", post(setup::probe))
        .route("/ollama-models", post(setup::ollama_models))
}

pub fn pages_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(pages::list_pages).post(pages::create_page))
        .route("/search", get(pages::search_pages))
        .route("/:id", get(pages::get_page).patch(pages::update_page))
        .route("/:id/versions", get(pages::get_page_versions))
        .route("/:id/history", get(pages::list_version_history))
        .route(
            "/:id/history/:history_id",
            get(pages::get_version_history_item),
        )
        .route(
            "/:id/history/:history_id/restore",
            post(pages::restore_version),
        )
        .route("/:id/publish", post(pages::publish_page))
        .route("/:id/draft", post(pages::draft_page))
        .route(
            "/:id/editor-conversation",
            get(editor::get_conversation).put(editor::put_conversation),
        )
        .route("/:id/export", get(export::export_page))
}

pub fn export_router() -> Router<Arc<AppState>> {
    Router::new().route("/", get(export::export_workspace))
}

pub fn collections_router() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/",
            get(collections::list_collections).post(collections::create_collection),
        )
        .route(
            "/:id",
            patch(collections::update_collection).delete(collections::delete_collection),
        )
}

pub fn admin_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/users", get(admin_users::list_users))
        .route("/users/invite", post(admin_users::invite))
        .route("/users/:id/deactivate", patch(admin_users::deactivate_user))
        .route("/workspace", get(admin_workspace::get_workspace))
        .route(
            "/workspace/regenerate-token",
            post(admin_workspace::regenerate_token),
        )
        .route("/workspace/llm", patch(admin_workspace::update_llm_config))
        .route("/workspace/reindex", post(admin_workspace::reindex))
        .route(
            "/analytics/mcp-queries",
            get(admin_analytics::get_mcp_analytics),
        )
}

/// Internal endpoints — no JWT auth, protected by network topology.
/// These are NOT nested under `/admin` and NOT behind the setup gate.
pub fn internal_router() -> Router<Arc<AppState>> {
    Router::new().route("/mcp-log", post(admin_analytics::log_mcp_query))
}

pub fn editor_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/draft", post(editor::draft))
        .route("/iterate", post(editor::iterate))
        // Sprint 11 phase A3 — editor-v2 WebSocket. Gated on
        // AppState.editor_v2_enabled inside the handler so the
        // route registers unconditionally but 404s when the flag
        // is off. That keeps the route table stable across
        // deploys and makes the flag check testable.
        .route("/ws", get(editor_ws::upgrade))
}
