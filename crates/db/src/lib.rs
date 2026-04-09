//! `historiador_db` — shared database clients for Historiador Doc.
//!
//! This library is consumed by both `historiador_api` and `historiador_mcp`.
//! It will expose:
//!   * A PgPool constructor from a URL
//!   * A `VectorStore` trait and an HTTP stub implementation (Sprint 2 replaces the stub)
//!   * An `embedded` sqlx migrator compiled from `migrations/`
//!
//! # Invariant (ADR-003)
//!
//! Migrations are only applied by `historiador_api` on boot.
//! The MCP server MUST NOT call `run_migrations` — the readonly Postgres
//! role lacks DDL privileges, and MCP has no business owning schema evolution.
//!
//! Sprint 1 ships an empty lib; Item 3 populates `postgres`, `vector_store`,
//! and the migrator.
