//! Re-export of the existing `LlmProbe` trait so the domain port
//! namespace is complete. The trait currently lives in
//! `crate::setup::llm_probe` and will be relocated to
//! `crate::infrastructure::llm::probe` in a later step.

pub use crate::setup::llm_probe::{LlmProbe, LlmProvider};
