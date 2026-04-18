//! `Cipher` port adapter wrapping the existing AES-256-GCM
//! implementation in [`crate::infrastructure::crypto::raw::Cipher`].

use crate::domain::error::ApplicationError;
use crate::domain::port::cipher::Cipher;
use crate::infrastructure::crypto::raw::Cipher as AesCipher;

pub struct AesGcmCipher {
    inner: AesCipher,
}

impl AesGcmCipher {
    pub fn new(inner: AesCipher) -> Self {
        Self { inner }
    }
}

impl Cipher for AesGcmCipher {
    fn encrypt(&self, plaintext: &str) -> Result<String, ApplicationError> {
        self.inner
            .encrypt(plaintext)
            .map_err(ApplicationError::Infrastructure)
    }

    fn decrypt(&self, ciphertext: &str) -> Result<String, ApplicationError> {
        self.inner
            .decrypt(ciphertext)
            .map_err(ApplicationError::Infrastructure)
    }
}
