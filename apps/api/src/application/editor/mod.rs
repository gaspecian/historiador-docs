//! Editor use cases — draft generation, iteration.

pub mod generate_draft;
pub mod iterate_draft;
pub mod prompts;

pub use generate_draft::{DraftStream, GenerateDraftCommand, GenerateDraftUseCase};
pub use iterate_draft::{IterateDraftCommand, IterateDraftUseCase, IterateStream};
