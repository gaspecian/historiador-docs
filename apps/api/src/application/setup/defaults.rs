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
