//! `historiador_api` — library surface for the Axum REST API.
//!
//! The API server exposes a thin lib so the `gen-openapi` binary can pull
//! the `ApiDoc` definition without re-implementing the router. In Sprint 1
//! only the health endpoint and empty route nests exist; Sprint 2 adds the
//! real feature routes.
