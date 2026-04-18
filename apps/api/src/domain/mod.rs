//! Domain layer — pure business concepts.
//!
//! No dependency on axum, sqlx, reqwest, or any infrastructure SDK may
//! cross the boundary into this module. Only `std`, `chrono`, `uuid`,
//! and other pure-logic crates are allowed. Anything else means a
//! leaked concern that should live in `infrastructure` or `presentation`.

pub mod entity;
pub mod error;
pub mod port;
pub mod value;
