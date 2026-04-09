//! `historiador_api` — library surface for the Axum REST API.
//!
//! Exposed as a library so the `gen-openapi` binary (stretch item 7)
//! can reach the `ApiDoc` struct without re-implementing the router.
//! In Sprint 1 only the health endpoint and empty route nests exist;
//! Sprint 2 adds the real feature routes behind the same module shape.

pub mod error;
pub mod health;
pub mod routes;
pub mod state;
