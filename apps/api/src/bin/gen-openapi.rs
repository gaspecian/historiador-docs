//! Standalone binary that emits `openapi.yaml` at the repository root.
//! Invoked by Turborepo's `gen:openapi` task via the root package.json
//! script: `cargo run --release -p historiador_api --bin gen-openapi`.

use historiador_api::openapi::ApiDoc;
use std::{fs, path::PathBuf};
use utoipa::OpenApi;

fn main() -> anyhow::Result<()> {
    let spec = ApiDoc::openapi().to_yaml()?;
    // CARGO_MANIFEST_DIR is apps/api → parent is apps → parent is repo root.
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("openapi.yaml");
    fs::write(&out, &spec)?;
    println!("wrote {} ({} bytes)", out.display(), spec.len());
    Ok(())
}
