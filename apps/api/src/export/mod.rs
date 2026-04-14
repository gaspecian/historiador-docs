//! Markdown export — streams a zip of published pages to admins and
//! single-page markdown downloads to any author.
//!
//! The zip is produced incrementally via `async_zip::tokio` feeding a
//! `DuplexStream`; the HTTP response body is the read half. No full
//! zip is ever held in memory — this lets us export workspaces with
//! thousands of pages without OOM'ing the API.

pub mod handlers;
