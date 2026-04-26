//! Boot-time vector store backfill — reconciles published page versions
//! in PostgreSQL into Chronik's `published-pages` topic.

pub mod service;
pub mod state;

pub use service::BackfillService;
pub use state::{shared_disabled, BackfillState, SharedBackfillState};
