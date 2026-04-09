//! `historiador_llm` — LLM provider abstraction.
//!
//! Sprint 2 will define an `LlmClient` trait and implementations for
//! OpenAI (`async-openai` or raw HTTP via `reqwest`), Anthropic (raw HTTP),
//! and Ollama (raw HTTP) per ADR-006. The workspace-level API key is
//! resolved by the API server from `workspaces.llm_api_key_encrypted`.
//!
//! Sprint 1 ships only the placeholder so the workspace compiles.
