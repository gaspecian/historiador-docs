//! Provider-appropriate default model names when the caller omits them.

use crate::infrastructure::llm::probe::LlmProvider;

pub fn generation_model(provider: LlmProvider) -> &'static str {
    match provider {
        LlmProvider::OpenAi => "gpt-4o-mini",
        LlmProvider::Anthropic => "claude-haiku-4-5-20251001",
        LlmProvider::Ollama => "llama3.1:8b",
        LlmProvider::Test => "stub",
    }
}

pub fn embedding_model(provider: LlmProvider) -> &'static str {
    match provider {
        // Anthropic has no embedding API; production falls back to
        // OpenAI with this model, so the default reflects that.
        LlmProvider::OpenAi | LlmProvider::Anthropic => "text-embedding-3-small",
        LlmProvider::Ollama => "nomic-embed-text",
        LlmProvider::Test => "stub",
    }
}
