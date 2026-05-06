//! Chunk-pipeline adapter — composes `historiador_chunker` with a
//! `VectorStore` behind the `ChunkPipeline` port. Chronik handles
//! embedding server-side; the app no longer constructs an embedding
//! client.

pub mod pipeline;

pub use pipeline::DefaultChunkPipeline;
