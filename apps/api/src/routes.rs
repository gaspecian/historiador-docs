//! Route group nests. Each sub-router is mounted into the top-level
//! app tree by [`crate::app::build_router`].

use axum::{
    routing::{get, patch, post},
    Router,
};
use std::sync::Arc;

use crate::admin;
use crate::auth;
use crate::collections;
use crate::editor;
use crate::pages;
use crate::setup;
use crate::state::AppState;

pub fn auth_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/login", post(auth::handlers::login))
        .route("/refresh", post(auth::handlers::refresh))
        .route("/logout", post(auth::handlers::logout))
        .route("/activate", post(auth::handlers::activate))
}

pub fn setup_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/init", post(setup::handler::init))
        .route("/probe", post(setup::handler::probe))
}

pub fn pages_router() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/",
            get(pages::handlers::list_pages).post(pages::handlers::create_page),
        )
        .route("/search", get(pages::handlers::search_pages))
        .route(
            "/:id",
            get(pages::handlers::get_page).patch(pages::handlers::update_page),
        )
        .route("/:id/versions", get(pages::handlers::get_page_versions))
        .route("/:id/history", get(pages::handlers::list_version_history))
        .route(
            "/:id/history/:history_id",
            get(pages::handlers::get_version_history_item),
        )
        .route(
            "/:id/history/:history_id/restore",
            post(pages::handlers::restore_version),
        )
        .route("/:id/publish", post(pages::handlers::publish_page))
        .route("/:id/draft", post(pages::handlers::draft_page))
}

pub fn collections_router() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/",
            get(collections::handlers::list_collections)
                .post(collections::handlers::create_collection),
        )
        .route(
            "/:id",
            patch(collections::handlers::update_collection)
                .delete(collections::handlers::delete_collection),
        )
}

pub fn admin_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/users", get(admin::users::list_users))
        .route("/users/invite", post(admin::users::invite))
        .route(
            "/users/:id/deactivate",
            patch(admin::users::deactivate_user),
        )
        .route("/workspace", get(admin::workspace::get_workspace))
        .route(
            "/workspace/regenerate-token",
            post(admin::workspace::regenerate_token),
        )
        .route(
            "/analytics/mcp-queries",
            get(admin::analytics::get_mcp_analytics),
        )
}

/// Internal endpoints — no JWT auth, protected by network topology.
/// These are NOT nested under `/admin` and NOT behind the setup gate.
pub fn internal_router() -> Router<Arc<AppState>> {
    Router::new().route("/mcp-log", post(admin::analytics::log_mcp_query))
}

pub fn editor_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/draft", post(editor::handlers::draft))
        .route("/iterate", post(editor::handlers::iterate))
}
