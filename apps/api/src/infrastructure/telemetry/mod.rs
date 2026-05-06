//! Lightweight in-process counters for Sprint 11 editor telemetry.
//!
//! Backed by atomic `u64`s so every handler path is lock-free. The
//! counters feed the success-metrics dashboard (sprint-11.md §356–363)
//! and the `canvas_overwrite_events == 0` invariant check. Exposed via
//! a `/internal/editor/metrics` route (wired in Phase A3/A4 when the
//! WebSocket handler and dispatcher exist).

pub mod editor;
