//! Versioned system-prompt loader for the AI editor.
//!
//! Reads the prompt file at boot so a restart is required to roll a
//! new version — simpler than hot-reload and acceptable given prompt
//! changes ship via deploy, not runtime config.
//!
//! Tuned template authored in Phase A7 (US-11.04).

use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// A loaded system prompt. `version` identifies which file produced the
/// body; `hash` is logged at startup so operators can correlate
/// deployments with prompt changes.
#[derive(Debug, Clone)]
pub struct LoadedPrompt {
    pub version: String,
    pub body: String,
    pub hash: String,
}

impl LoadedPrompt {
    /// In-memory stub for tests. Lets integration tests build an
    /// `AppState` without touching the filesystem.
    pub fn for_test() -> Self {
        let body = "# test persona\n\nHello.\n".to_string();
        Self {
            version: "test".to_string(),
            hash: "test".to_string(),
            body,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PromptError {
    #[error("prompt file not found at {path}: {source}")]
    NotFound {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("prompt file at {path} is empty")]
    Empty { path: PathBuf },
}

/// Resolve and load the agent prompt. The version comes from
/// `PROMPT_VERSION` (default `v1`); the search root comes from
/// `PROMPT_DIR` (default `prompts/agent`). CWD-relative so `cargo run`
/// from the repo root works; production containers set `PROMPT_DIR` to
/// an absolute path.
pub fn load_agent_prompt(version: &str, dir: &Path) -> Result<LoadedPrompt, PromptError> {
    let path = dir.join(format!("{version}.md"));
    let body = fs::read_to_string(&path).map_err(|e| PromptError::NotFound {
        path: path.clone(),
        source: e,
    })?;

    if body.trim().is_empty() {
        return Err(PromptError::Empty { path });
    }

    let mut hasher = Sha256::new();
    hasher.update(body.as_bytes());
    let hash = hex_short(&hasher.finalize());

    Ok(LoadedPrompt {
        version: version.to_string(),
        body,
        hash,
    })
}

fn hex_short(bytes: &[u8]) -> String {
    bytes.iter().take(8).map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_a_prompt_and_produces_a_hash() {
        let tmp = tempdir();
        let path = tmp.path().join("v1.md");
        fs::write(&path, "# persona\n\nHello.\n").unwrap();

        let loaded = load_agent_prompt("v1", tmp.path()).unwrap();
        assert_eq!(loaded.version, "v1");
        assert!(loaded.body.contains("persona"));
        assert_eq!(loaded.hash.len(), 16);
    }

    #[test]
    fn rejects_missing_file() {
        let tmp = tempdir();
        let err = load_agent_prompt("vnope", tmp.path()).unwrap_err();
        assert!(matches!(err, PromptError::NotFound { .. }));
    }

    #[test]
    fn rejects_empty_file() {
        let tmp = tempdir();
        let path = tmp.path().join("v1.md");
        fs::write(&path, "   \n\n").unwrap();
        let err = load_agent_prompt("v1", tmp.path()).unwrap_err();
        assert!(matches!(err, PromptError::Empty { .. }));
    }

    fn tempdir() -> tempfile_shim::TempDir {
        tempfile_shim::TempDir::new()
    }

    // Local shim so the crate does not take a dependency on `tempfile`
    // just for this one test. The shim creates a unique subdir inside
    // the target tmp dir and cleans up on Drop.
    mod tempfile_shim {
        use std::path::{Path, PathBuf};
        use std::sync::atomic::{AtomicU32, Ordering};
        use std::time::{SystemTime, UNIX_EPOCH};

        pub struct TempDir(PathBuf);

        static COUNTER: AtomicU32 = AtomicU32::new(0);

        impl TempDir {
            pub fn new() -> Self {
                let mut path = std::env::temp_dir();
                let ns = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0);
                let n = COUNTER.fetch_add(1, Ordering::Relaxed);
                path.push(format!("historiador_prompts_{ns}_{n}"));
                std::fs::create_dir_all(&path).unwrap();
                TempDir(path)
            }
            pub fn path(&self) -> &Path {
                &self.0
            }
        }

        impl Drop for TempDir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }
}
