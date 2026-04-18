//! Presentation layer — Axum handlers, DTOs, extractors, middleware,
//! error mapping, OpenAPI registry.
//!
//! Depends on `crate::application` and `crate::domain`. The
//! composition of adapters (sqlx repos, Chronik client, JWT issuer,
//! etc.) into use-case objects lives here at `state::UseCases` so
//! `main.rs` stays thin.

pub mod dto;
pub mod error;
pub mod extractor;
pub mod handler;
pub mod middleware;
pub mod openapi;
pub mod state;

pub use error::ApiError;
pub use openapi::ApiDoc;
pub use state::{BuildDeps, UseCases};
