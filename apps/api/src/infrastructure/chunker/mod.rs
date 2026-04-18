//! Chunk-pipeline adapter — composes `historiador_chunker` with an
//! `EmbeddingClient` and a `VectorStore` behind the `ChunkPipeline`
//! port.

pub mod pipeline;

pub use pipeline::DefaultChunkPipeline;
