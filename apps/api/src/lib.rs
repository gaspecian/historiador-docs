//! `historiador_api` — library surface for the Axum REST API.
//!
//! Exposed as a library so the `gen-openapi` binary can reach the
//! `ApiDoc` struct without re-implementing the router, and so that
//! integration tests can spin up a fully configured app without
//! shelling out through `main`.
//!
//! # Layout
//!
//! Clean Architecture layers:
//!
//! - [`domain`] — pure entities, value objects, port traits
//! - [`application`] — use cases orchestrating ports
//! - [`infrastructure`] — adapters implementing ports (sqlx, Chronik,
//!   JWT, AES-GCM, HTTP clients, …)
//! - [`presentation`] — Axum handlers, DTOs, extractors, middleware,
//!   error mapping, OpenAPI registry
//!
//! [`app`] + [`routes`] + [`state`] wire the layers together. [`util`]
//! holds tiny shared helpers that don't fit any other home.

pub mod app;
pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod presentation;
pub mod routes;
pub mod state;
pub mod util;
