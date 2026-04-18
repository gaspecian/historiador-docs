//! Axum extractors — identity / authorization pulled out of the
//! incoming request.

pub mod auth_user;

pub use auth_user::AuthUser;
