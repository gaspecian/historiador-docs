//! Setup use cases — first-run installation.

pub mod bcp47;
pub mod defaults;
pub mod initialize_installation;
pub mod list_ollama_models;
pub mod probe_llm;

pub use initialize_installation::{
    InitializeInstallationCommand, InitializeInstallationUseCase, InstallationInitialized,
};
pub use list_ollama_models::{ListOllamaModelsUseCase, OllamaModel};
pub use probe_llm::{ProbeLlmResult, ProbeLlmUseCase};
