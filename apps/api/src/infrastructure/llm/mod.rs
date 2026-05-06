//! LLM infrastructure adapters.
//!
//! - [`probe`] — `LlmProbe` trait + `HttpLlmProbe` implementation
//!   used by the setup wizard and admin LLM patch to validate a
//!   provider credential before it is persisted.

pub mod probe;
