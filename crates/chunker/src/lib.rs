//! `historiador_chunker` — structure-aware markdown chunker.
//!
//! Sprint 2 will implement the algorithm specified in ADR-002:
//! parse markdown to a `comrak` AST and emit heading-delimited chunks
//! (with paragraph-boundary fallback for oversized sections). Code blocks,
//! tables, and lists are atomic AST nodes and are never split.
//!
//! Sprint 1 ships only the placeholder so the workspace compiles.
