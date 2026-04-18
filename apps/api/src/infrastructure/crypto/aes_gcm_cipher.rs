//! `Cipher` port adapter wrapping the existing AES-256-GCM
//! implementation in [`crate::crypto::Cipher`].

use crate::crypto::Cipher as AesCipher;
use crate::domain::error::ApplicationError;
use crate::domain::port::cipher::Cipher;

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
