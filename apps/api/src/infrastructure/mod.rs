//! Infrastructure layer — adapters that implement domain ports using
//! concrete technologies (sqlx, reqwest, aes-gcm, jsonwebtoken, …).
//!
//! Depends on `crate::domain`. Must never depend on `crate::presentation`.

pub mod auth;
pub mod chronik;
pub mod chunker;
pub mod config;
pub mod crypto;
pub mod llm;
pub mod persistence;
pub mod prompts;
pub mod system;
pub mod telemetry;
pub mod token;
