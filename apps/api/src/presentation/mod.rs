//! Presentation layer — Axum handlers, DTOs, extractors, middleware,
//! error mapping, OpenAPI registry.
//!
//! Depends on `crate::application` and `crate::domain`. Never on
//! `crate::infrastructure` directly — construction of adapters belongs
//! in `main.rs` (the composition root).
//!
//! During the scaffolding step this module is empty; the existing
//! `crate::{auth,pages,collections,admin,editor,setup,export}` trees
//! remain the source of truth until the presentation rewire step.

pub mod dto;
pub mod extractor;
pub mod handler;
pub mod middleware;
