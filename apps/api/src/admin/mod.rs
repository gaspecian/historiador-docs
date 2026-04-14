//! Admin-only endpoints. Every handler in this tree checks
//! `require_role(Admin)` as its first step.

pub mod analytics;
pub mod users;
pub mod workspace;
