//! MCP-layer error. Kept minimal — the only failure modes a query
//! use case surfaces are "bad input" (empty query etc.) and "infra
//! failure" (vector store / embedding / metadata lookup broke).

use thiserror::Error;

#[allow(dead_code)]
#[derive(Debug, Error)]
pub enum McpError {
    /// Not yet constructed — the current single use case only uses
    /// the infrastructure path. Kept in the enum so new read-only
    /// use cases can surface validation errors without widening the
    /// signature.
    #[error("{0}")]
    Validation(String),

    #[error(transparent)]
    Infrastructure(#[from] anyhow::Error),
}
