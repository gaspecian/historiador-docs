//! Symmetric cipher port — encrypt/decrypt sensitive workspace
//! config (LLM API keys). The application layer depends on this
//! trait, not on `aes-gcm` directly.

use crate::domain::error::ApplicationError;

pub trait Cipher: Send + Sync {
    fn encrypt(&self, plaintext: &str) -> Result<String, ApplicationError>;

    fn decrypt(&self, ciphertext: &str) -> Result<String, ApplicationError>;
}
