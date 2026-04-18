//! Auth-related infrastructure: JWT access-token codec and
//! random/hashed refresh-/invite-token utilities. Both are thin
//! wrappers over third-party crates (`jsonwebtoken`, `sha2`) and do
//! not depend on any domain type.

pub mod jwt;
pub mod refresh_tokens;
