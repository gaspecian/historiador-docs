//! Port traits — abstractions the application layer depends on.
//!
//! Implementations live under `crate::infrastructure`. Traits here are
//! narrow and use-case-oriented, not reflections of the database
//! schema: a `publish_page` use case should not have to know about
//! five different repositories.
//!
//! Re-exports existing trait ports from the shared crates so
//! application code can depend on the `domain::port::*` namespace
//! uniformly, without caring whether a port lives in this repo or in
//! `crates/llm` / `crates/db`.

pub mod chunk_pipeline;
pub mod cipher;
pub mod clock;
pub mod collection_repository;
pub mod event_producer;
pub mod export_repository;
pub mod id_generator;
pub mod installation_repository;
pub mod llm_probe;
pub mod page_repository;
pub mod query_analytics;
pub mod session_repository;
pub mod token_issuer;
pub mod user_repository;
pub mod version_history_repository;
pub mod workspace_repository;

// Re-exports of traits that already exist in shared crates.
pub use historiador_db::vector_store::VectorStore;
pub use historiador_llm::{EmbeddingClient, TextGenerationClient};
