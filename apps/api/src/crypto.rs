//! Symmetric encryption for secrets stored at rest.
//!
//! Currently used for `workspaces.llm_api_key_encrypted`. AES-256-GCM
//! with a 256-bit key provided via `APP_ENCRYPTION_KEY` (base64).
//! Ciphertext format on disk: `base64(nonce(12) || ciphertext || tag(16))`.
//!
//! **Key rotation is not supported in v1.** Rotating the key means
//! re-encrypting every stored secret and requires a dedicated migration
//! path; we accept this limitation and document it in the plan.

use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{AeadCore, Aes256Gcm, Key, Nonce};
use anyhow::{anyhow, Context};
use base64::Engine;

/// Thin wrapper around an `Aes256Gcm` cipher. Cheap to clone — the
/// inner state is just the expanded key schedule.
#[derive(Clone)]
pub struct Cipher {
    inner: Aes256Gcm,
}

impl Cipher {
    /// Load a cipher from a base64-encoded 32-byte key (the shape of
    /// `APP_ENCRYPTION_KEY`). Returns an error if the key is missing,
    /// not valid base64, or not exactly 32 bytes.
    pub fn from_base64(b64: &str) -> anyhow::Result<Self> {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64.as_bytes())
            .context("APP_ENCRYPTION_KEY is not valid base64")?;
        if bytes.len() != 32 {
            return Err(anyhow!(
                "APP_ENCRYPTION_KEY must decode to exactly 32 bytes (got {})",
                bytes.len()
            ));
        }
        let key = Key::<Aes256Gcm>::from_slice(&bytes);
        Ok(Self {
            inner: Aes256Gcm::new(key),
        })
    }

    /// Encrypt a UTF-8 string. The returned value is
    /// `base64(nonce || ciphertext || tag)`.
    pub fn encrypt(&self, plaintext: &str) -> anyhow::Result<String> {
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ct = self
            .inner
            .encrypt(&nonce, plaintext.as_bytes())
            .map_err(|e| anyhow!("aes-gcm encrypt failed: {e}"))?;
        let mut out = Vec::with_capacity(nonce.len() + ct.len());
        out.extend_from_slice(nonce.as_slice());
        out.extend_from_slice(&ct);
        Ok(base64::engine::general_purpose::STANDARD.encode(out))
    }

    /// Decrypt a string previously produced by [`encrypt`]. Errors on
    /// any tamper (bad tag, wrong key, truncated payload).
    pub fn decrypt(&self, encoded: &str) -> anyhow::Result<String> {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded.as_bytes())
            .context("ciphertext is not valid base64")?;
        if bytes.len() < 12 + 16 {
            return Err(anyhow!("ciphertext too short"));
        }
        let (nonce_bytes, ct) = bytes.split_at(12);
        let nonce = Nonce::from_slice(nonce_bytes);
        let pt = self
            .inner
            .decrypt(nonce, ct)
            .map_err(|e| anyhow!("aes-gcm decrypt failed: {e}"))?;
        String::from_utf8(pt).context("decrypted bytes are not valid utf-8")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_cipher() -> Cipher {
        // 32-byte key of zeros — test fixture only, never use in prod.
        let key_b64 = base64::engine::general_purpose::STANDARD.encode([0u8; 32]);
        Cipher::from_base64(&key_b64).unwrap()
    }

    #[test]
    fn round_trip_preserves_plaintext() {
        let c = test_cipher();
        let ct = c.encrypt("sk-test-abcdef").unwrap();
        assert_eq!(c.decrypt(&ct).unwrap(), "sk-test-abcdef");
    }

    #[test]
    fn ciphertext_is_randomized_per_call() {
        let c = test_cipher();
        let a = c.encrypt("same").unwrap();
        let b = c.encrypt("same").unwrap();
        assert_ne!(a, b, "nonce reuse would be catastrophic for AES-GCM");
    }

    #[test]
    fn tampered_ciphertext_fails_decrypt() {
        let c = test_cipher();
        let ct = c.encrypt("secret").unwrap();
        let mut bytes = base64::engine::general_purpose::STANDARD
            .decode(ct.as_bytes())
            .unwrap();
        // Flip a bit in the ciphertext body (past the 12-byte nonce).
        bytes[15] ^= 0x01;
        let tampered = base64::engine::general_purpose::STANDARD.encode(&bytes);
        assert!(c.decrypt(&tampered).is_err());
    }

    #[test]
    fn rejects_non_32_byte_key() {
        let short = base64::engine::general_purpose::STANDARD.encode([0u8; 16]);
        assert!(Cipher::from_base64(&short).is_err());
    }
}
