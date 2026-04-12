//! `historiador_api` — library surface for the Axum REST API.
//!
//! Exposed as a library so the `gen-openapi` binary can reach the
//! `ApiDoc` struct without re-implementing the router, and so that
//! integration tests can spin up a fully configured app without
//! shelling out through `main`.

pub mod admin;
pub mod auth;
pub mod collections;
pub mod crypto;
pub mod error;
pub mod health;
pub mod middleware;
pub mod openapi;
pub mod pages;
pub mod routes;
pub mod setup;
pub mod state;
pub mod util;

pub mod app;
