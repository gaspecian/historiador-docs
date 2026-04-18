//! Authentication and authorization primitives.
//!
//! - [`jwt`] — HS256 signing of access tokens
//! - [`tokens`] — opaque refresh + invite token generation and hashing
//! - [`extractor`] — `AuthUser` Axum extractor (pulls Bearer token, decodes JWT)
//! - [`handlers`] — `/auth/{login,refresh,logout,activate}` HTTP handlers
//!
//! Authorization lives with the use cases — see
//! `crate::domain::value::Actor::require_role`. Handlers project
//! `AuthUser` into `Actor` via `AuthUser::as_actor()` before invoking
//! a use case; the use case enforces the required role.
//!
//! The access-token path is stateless (JWT verify only, no DB hit).
//! The refresh-token path is stateful (session row lookup + rotation).

pub mod extractor;
pub mod handlers;
pub mod jwt;
pub mod tokens;
