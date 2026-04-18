//! Opaque token generation and hashing for refresh tokens and
//! invite tokens. These tokens are **not** JWTs — they are random
//! 256-bit values stored as sha256 hashes in the database, so a
//! compromised database read never leaks working tokens.

use base64::Engine;
use rand::RngCore;
use sha2::{Digest, Sha256};

/// Refresh token lifetime: 7 days. After that, the user must log in
/// with their password again.
pub const REFRESH_TOKEN_TTL_DAYS: i64 = 7;

/// Invite token lifetime: 7 days. Non-activated pending users past
/// this window must be re-invited.
pub const INVITE_TOKEN_TTL_DAYS: i64 = 7;

/// Generate a random 256-bit token. Returns
/// `(plaintext_base64url, sha256_hex_hash)`. The plaintext is
/// returned **only once** — to the user who triggered the flow —
/// and the hash is what gets stored.
pub fn generate() -> (String, String) {
    let mut buf = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut buf);
    let plaintext = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buf);
    let hash = sha256_hex(&plaintext);
    (plaintext, hash)
}

/// Hash a plaintext token for lookup. Deterministic (no salt) so
/// `WHERE invite_token_hash = $1` works as an indexed point query.
pub fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let out = hasher.finalize();
    hex_encode(&out)
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_tokens_are_unique() {
        let (a, _) = generate();
        let (b, _) = generate();
        assert_ne!(a, b);
    }

    #[test]
    fn plaintext_hashes_back_to_stored_hash() {
        let (plaintext, hash) = generate();
        assert_eq!(sha256_hex(&plaintext), hash);
    }

    #[test]
    fn hash_is_deterministic() {
        assert_eq!(sha256_hex("hello"), sha256_hex("hello"));
    }

    #[test]
    fn hash_differs_by_input() {
        assert_ne!(sha256_hex("hello"), sha256_hex("world"));
    }

    #[test]
    fn generated_plaintext_has_sufficient_entropy() {
        let (plaintext, _) = generate();
        // 32 random bytes → 43 base64url chars (no padding)
        assert!(plaintext.len() >= 40);
    }
}
