//! MCP application layer — use cases for read-only query operations.

pub mod error;
pub mod port;
pub mod search_chunks;

pub use error::McpError;
pub use search_chunks::{SearchChunksCommand, SearchChunksUseCase};
