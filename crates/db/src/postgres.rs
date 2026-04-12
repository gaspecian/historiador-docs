//! Postgres query helpers, grouped by entity.
//!
//! Each submodule owns the queries for one table plus its row struct.
//! All functions return `anyhow::Result` to match the crate's existing
//! style. Submodules that take `&mut Transaction<'_, Postgres>` are
//! intended to be composed inside a transactional workflow (setup,
//! invite activation); those taking `&PgPool` are standalone queries.

pub mod chunks;
pub mod collections;
pub mod installation;
pub mod mcp_queries;
pub mod page_versions;
pub mod pages;
pub mod sessions;
pub mod users;
pub mod workspaces;
