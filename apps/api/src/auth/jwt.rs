//! Access-token signing and verification (HS256 JWT).
//!
//! Access tokens are stateless: the middleware only needs the shared
//! secret to verify them. Revocation is handled by keeping token
//! lifetime short (1h) and rotating via refresh tokens; there is no
//! per-request DB lookup on the hot path.

use chrono::{Duration, Utc};
use historiador_db::postgres::users::Role;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Access token lifetime. Short enough that revocation-by-expiry is
/// acceptable in place of server-side session lookups on each request.
pub const ACCESS_TOKEN_TTL_SECONDS: i64 = 3600;

/// JWT claims. `sub` is the user id (UUID); `wsid` is the workspace id;
/// `role` is serialized as lowercase ("admin"/"author"/"viewer") so the
/// token is self-contained for RBAC decisions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,
    pub wsid: Uuid,
    pub role: Role,
    pub exp: i64,
    pub iat: i64,
    pub jti: Uuid,
}

impl Claims {
    pub fn new(user_id: Uuid, workspace_id: Uuid, role: Role) -> Self {
        let now = Utc::now();
        Self {
            sub: user_id,
            wsid: workspace_id,
            role,
            exp: (now + Duration::seconds(ACCESS_TOKEN_TTL_SECONDS)).timestamp(),
            iat: now.timestamp(),
            jti: Uuid::new_v4(),
        }
    }
}

pub fn encode_token(claims: &Claims, secret: &[u8]) -> anyhow::Result<String> {
    let token = encode(
        &Header::default(),
        claims,
        &EncodingKey::from_secret(secret),
    )?;
    Ok(token)
}

pub fn decode_token(token: &str, secret: &[u8]) -> anyhow::Result<Claims> {
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret),
        &Validation::default(),
    )?;
    Ok(data.claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &[u8] = b"test-secret-at-least-32-bytes-long-for-hs256";

    #[test]
    fn encode_then_decode_round_trips() {
        let claims = Claims::new(Uuid::new_v4(), Uuid::new_v4(), Role::Admin);
        let tok = encode_token(&claims, SECRET).unwrap();
        let back = decode_token(&tok, SECRET).unwrap();
        assert_eq!(back.sub, claims.sub);
        assert_eq!(back.wsid, claims.wsid);
        assert_eq!(back.role, claims.role);
    }

    #[test]
    fn wrong_secret_fails_decode() {
        let claims = Claims::new(Uuid::new_v4(), Uuid::new_v4(), Role::Author);
        let tok = encode_token(&claims, SECRET).unwrap();
        assert!(decode_token(&tok, b"different-secret-different-secret-x").is_err());
    }

    #[test]
    fn expired_token_fails_decode() {
        let mut claims = Claims::new(Uuid::new_v4(), Uuid::new_v4(), Role::Viewer);
        // jsonwebtoken's default validation allows 60s of clock-skew
        // leeway, so an expired token must be well past that window.
        claims.exp = Utc::now().timestamp() - 600;
        let tok = encode_token(&claims, SECRET).unwrap();
        assert!(decode_token(&tok, SECRET).is_err());
    }
}
