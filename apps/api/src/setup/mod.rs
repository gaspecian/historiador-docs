//! First-run setup wizard. `POST /setup/init` is the only endpoint
//! callable before the installation is configured; it creates the
//! workspace, the admin user, and marks the install complete.

pub mod handler;
pub mod llm_probe;
