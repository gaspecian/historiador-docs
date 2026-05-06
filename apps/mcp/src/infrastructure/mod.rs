//! MCP infrastructure adapters — only read-capable ones are in scope.

pub mod postgres_readonly;

pub use postgres_readonly::PostgresChunkMetadataReader;
