//! Thin HTTP handlers. Each function: extract → build command → call
//! use case → map response. No business logic.

pub mod admin;
pub mod auth;
pub mod collections;
pub mod editor;
pub mod export;
pub mod health;
pub mod pages;
pub mod setup;
