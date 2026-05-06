//! Application layer — use cases that orchestrate domain ports.
//!
//! Depends only on `crate::domain`. Never imports axum, sqlx, reqwest,
//! or any HTTP / persistence concrete type. Use cases receive their
//! collaborators as `Arc<dyn SomePort>` fields on a struct.

pub mod admin;
pub mod auth;
pub mod collections;
pub mod editor;
pub mod error;
pub mod export;
pub mod pages;
pub mod setup;
