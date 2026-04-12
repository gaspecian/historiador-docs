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
    Router::new().route("/init", post(setup::handler::init))
}

pub fn pages_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", post(pages::handlers::create_page))
        .route("/:id", get(pages::handlers::get_page).patch(pages::handlers::update_page))
        .route("/:id/publish", post(pages::handlers::publish_page))
        .route("/:id/draft", post(pages::handlers::draft_page))
}

pub fn collections_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(collections::handlers::list_collections).post(collections::handlers::create_collection))
        .route("/:id", patch(collections::handlers::update_collection).delete(collections::handlers::delete_collection))
}

pub fn admin_router() -> Router<Arc<AppState>> {
    Router::new().route("/users/invite", post(admin::users::invite))
}

pub fn editor_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/draft", post(editor::handlers::draft))
        .route("/iterate", post(editor::handlers::iterate))
}
