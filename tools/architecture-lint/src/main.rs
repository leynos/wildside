//! CLI entry point for the repo-local architecture lint.

use std::fs;
use std::io::{self, Write};
use std::path::Path;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let backend_dir = repo_root().join("backend");
    match architecture_lint::lint_backend_sources(&backend_dir) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            let mut stderr = io::stderr().lock();
            if let Err(write_err) = writeln!(stderr, "{err}") {
                drop(write_err);
            }
            ExitCode::FAILURE
        }
    }
}

fn repo_root() -> PathBuf {
    let from_cwd = std::env::current_dir().ok();
    let from_manifest = Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")));

    from_cwd
        .as_deref()
        .and_then(find_workspace_root)
        .or_else(|| from_manifest.as_deref().and_then(find_workspace_root))
        .expect("unable to locate workspace root (directory containing a workspace Cargo.toml)")
}

fn find_workspace_root(start: &Path) -> Option<PathBuf> {
    let mut current = Some(start);
    while let Some(dir) = current {
        let manifest = dir.join("Cargo.toml");
        if manifest.is_file() && cargo_toml_declares_workspace(&manifest) {
            return Some(dir.to_path_buf());
        }
        current = dir.parent();
    }
    None
}

fn cargo_toml_declares_workspace(path: &Path) -> bool {
    fs::read_to_string(path)
        .ok()
        .is_some_and(|contents| contents.contains("[workspace]"))
}
