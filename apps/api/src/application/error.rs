//! `ApplicationError` lives in `crate::domain::error` so port traits
//! (in `crate::domain::port`) can reference it without a cycle.
//! This re-export keeps `crate::application::error::ApplicationError`
//! as a stable path for use-case code.

pub use crate::domain::error::ApplicationError;
