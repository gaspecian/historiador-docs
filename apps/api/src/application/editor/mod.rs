//! Editor use cases — draft generation, iteration.

pub mod generate_draft;
pub mod iterate_draft;

pub use generate_draft::{DraftStream, GenerateDraftCommand, GenerateDraftUseCase};
pub use iterate_draft::{IterateDraftCommand, IterateDraftUseCase, IterateStream};
