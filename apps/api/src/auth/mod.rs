//! Authentication and authorization primitives.
//!
//! - [`jwt`] — HS256 signing of access tokens
//! - [`tokens`] — opaque refresh + invite token generation and hashing
//! - [`extractor`] — `AuthUser` Axum extractor (pulls Bearer token, decodes JWT)
//! - [`rbac`] — role-hierarchy middleware layer
//! - [`handlers`] — `/auth/{login,refresh,logout,activate}` HTTP handlers
//!
//! The access-token path is stateless (JWT verify only, no DB hit).
//! The refresh-token path is stateful (session row lookup + rotation).

pub mod extractor;
pub mod handlers;
pub mod jwt;
pub mod rbac;
pub mod tokens;
