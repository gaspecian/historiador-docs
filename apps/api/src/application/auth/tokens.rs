//! Token-pair output returned by login/refresh use cases. Captures
//! exactly what the HTTP layer needs to build a `TokenResponse` — no
//! more (e.g. no DB rows, no Session domain type).

#[derive(Debug, Clone)]
pub struct IssuedTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in_seconds: i64,
}
