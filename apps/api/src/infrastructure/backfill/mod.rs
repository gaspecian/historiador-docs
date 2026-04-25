//! Boot-time vector store backfill — reconciles published page versions
//! in PostgreSQL into Chronik's `published-pages` topic.

pub mod state;

pub use state::{shared_disabled, BackfillState, SharedBackfillState};
