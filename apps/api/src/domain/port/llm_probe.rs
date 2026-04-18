//! Re-export of the existing `LlmProbe` trait so the domain port
//! namespace is complete. The trait currently lives in
//! `crate::infrastructure::llm::probe` and will be relocated to
//! `crate::infrastructure::llm::probe` in a later step.

pub use crate::infrastructure::llm::probe::{LlmProbe, LlmProvider};
