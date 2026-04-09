//! `VectorStore` trait + HTTP-based VexFS client stub.
//!
//! Sprint 1 ships only the trait surface and an empty `reqwest`-backed
//! implementation so the rest of the stack compiles and the api/mcp
//! containers can health-check VexFS. Real wire integration is deferred
//! to Sprint 2 — at that point the [`search`](VectorStore::search) stub
//! is replaced with real similarity-search calls against VexFS's HTTP
//! API (port 7680, currently one of three dialects: Chroma, Qdrant, or
//! native VexFS — to be chosen during Sprint 2 planning).
//!
//! # Invariant (ADR-001)
//!
//! VexFS is the retrieval source of truth for chunk embeddings.
//! Postgres stores only `chunks.vexfs_ref` — an opaque pointer that
//! [`VectorStore`] implementations interpret. Never duplicate embeddings
//! into Postgres.

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum VectorStoreError {
    #[error("vexfs http error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("not implemented — Sprint 2 wire integration pending")]
    NotImplemented,
}

/// Opaque pointer to a chunk embedding stored in the vector store,
/// paired with a relevance score from a similarity search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkRef {
    pub vexfs_ref: String,
    pub score: f32,
}

/// Abstraction over the vector store. Swappable so the API and MCP
/// server can be tested against an in-memory fake, and so a migration
/// to a different backend (e.g. Qdrant) never touches call sites.
#[async_trait]
pub trait VectorStore: Send + Sync {
    /// Returns `Ok(true)` if the underlying store responds to a health
    /// probe. Used by the api/mcp health endpoints to surface upstream
    /// availability.
    async fn health(&self) -> Result<bool, VectorStoreError>;

    /// Top-k similarity search. Sprint 1 returns
    /// [`VectorStoreError::NotImplemented`]; Sprint 2 wires this to VexFS.
    async fn search(&self, query: &str, k: usize) -> Result<Vec<ChunkRef>, VectorStoreError>;
}

/// HTTP client for VexFS's unified REST server.
///
/// Talks to VexFS via `reqwest` (no first-party Rust client crate exists
/// upstream as of Sprint 1 — see ADR-001 Sprint 1 mitigation). The base
/// URL typically looks like `http://vexfs:7680` inside docker-compose.
pub struct HttpVexfsClient {
    base_url: String,
    http: Client,
}

impl HttpVexfsClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            http: Client::new(),
        }
    }
}

#[async_trait]
impl VectorStore for HttpVexfsClient {
    async fn health(&self) -> Result<bool, VectorStoreError> {
        // VexFS exposes `/api/v1/version` as its public liveness probe;
        // the container's Dockerfile HEALTHCHECK uses the same endpoint.
        let url = format!("{}/api/v1/version", self.base_url);
        let resp = self.http.get(url).send().await?;
        Ok(resp.status().is_success())
    }

    async fn search(&self, _query: &str, _k: usize) -> Result<Vec<ChunkRef>, VectorStoreError> {
        Err(VectorStoreError::NotImplemented)
    }
}
