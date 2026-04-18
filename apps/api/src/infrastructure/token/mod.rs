//! Token-issuer adapter — JWT issue/verify. Thin wrapper over the
//! existing helpers in `crate::auth::jwt`.

pub mod jwt_issuer;

pub use jwt_issuer::JwtTokenIssuer;
