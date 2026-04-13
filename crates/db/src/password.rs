//! Password hashing and verification using Argon2id.
//!
//! Used by the API server for admin account creation (setup wizard),
//! user activation (invite flow), and login. The MCP server does not
//! authenticate users and must never depend on this module.

use argon2::password_hash::{
    rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
};
use argon2::Argon2;

/// Hash a plaintext password with Argon2id and a random salt.
///
/// Uses `Argon2::default()` parameters (m=19456 KiB, t=2, p=1). On
/// modern hardware this targets ~50ms per hash, which is the upper
/// bound we want to accept inside a login request.
pub fn hash(plaintext: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon = Argon2::default();
    let hash = argon
        .hash_password(plaintext.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("argon2 hash failed: {e}"))?
        .to_string();
    Ok(hash)
}

/// Verify a plaintext password against a stored Argon2 PHC string.
///
/// Returns `Ok(true)` on match, `Ok(false)` on mismatch, and `Err`
/// only if the stored hash is malformed (which indicates a bug or
/// data corruption, not a failed login).
pub fn verify(plaintext: &str, stored_hash: &str) -> anyhow::Result<bool> {
    let parsed = PasswordHash::new(stored_hash)
        .map_err(|e| anyhow::anyhow!("malformed password hash: {e}"))?;
    Ok(Argon2::default()
        .verify_password(plaintext.as_bytes(), &parsed)
        .is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_then_verify_accepts_original() {
        let h = hash("correct horse battery staple").unwrap();
        assert!(verify("correct horse battery staple", &h).unwrap());
    }

    #[test]
    fn verify_rejects_wrong_password() {
        let h = hash("correct horse battery staple").unwrap();
        assert!(!verify("Tr0ub4dor&3", &h).unwrap());
    }

    #[test]
    fn hash_is_salted_so_two_hashes_of_same_input_differ() {
        let a = hash("same").unwrap();
        let b = hash("same").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn verify_errors_on_malformed_hash() {
        assert!(verify("anything", "not-a-real-phc-string").is_err());
    }
}
