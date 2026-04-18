//! Crypto adapters — the AES-256-GCM primitive and a `Cipher` port
//! adapter that wraps it behind the domain trait.

pub mod aes_gcm_cipher;
pub mod raw;

pub use aes_gcm_cipher::AesGcmCipher;
pub use raw::Cipher;
