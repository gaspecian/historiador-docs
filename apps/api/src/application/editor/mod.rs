//! Editor use cases — draft generation, iteration, conversation
//! persistence.

pub mod block_ops;
pub mod context;
pub mod conversation;
pub mod generate_draft;
pub mod intake;
pub mod iterate_draft;
pub mod outline;
pub mod prompt_template;
pub mod prompts;

pub use conversation::{
    LoadEditorConversationUseCase, SaveEditorConversationCommand, SaveEditorConversationUseCase,
};
pub use generate_draft::{DraftStream, GenerateDraftCommand, GenerateDraftUseCase};
pub use iterate_draft::{IterateDraftCommand, IterateDraftUseCase, IterateStream};
