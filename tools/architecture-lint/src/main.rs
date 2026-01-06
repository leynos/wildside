//! CLI entry point for the repo-local architecture lint.
//!
//! The main function establishes the capability boundary: it discovers the
//! workspace root using ambient filesystem access, then opens a capability
//! handle to the `backend/` directory which is passed to the lint library.

use std::fmt;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use cap_std::ambient_authority;
use cap_std::fs::Dir;

fn main() -> ExitCode {
    let backend_path = match repo_root() {
        Ok(root) => root.join("backend"),
        Err(err) => {
            let _ = writeln!(io::stderr().lock(), "{err}");
            return ExitCode::FAILURE;
        }
    };
    let backend_dir = match Dir::open_ambient_dir(&backend_path, ambient_authority()) {
        Ok(dir) => dir,
        Err(err) => {
            let _ = writeln!(
                io::stderr().lock(),
                "failed to open backend directory: {err}"
            );
            return ExitCode::FAILURE;
        }
    };
    match architecture_lint::lint_backend_sources(&backend_dir) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            let mut stderr = io::stderr().lock();
            let _ = writeln!(stderr, "{err}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct RepoRootError;

impl fmt::Display for RepoRootError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unable to locate workspace root (directory containing a workspace Cargo.toml)"
        )
    }
}

impl std::error::Error for RepoRootError {}

fn repo_root() -> Result<PathBuf, RepoRootError> {
    let from_env = std::env::var("CARGO_WORKSPACE_DIR").ok().map(PathBuf::from);
    let from_cwd = std::env::current_dir().ok();
    let from_manifest = Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")));

    from_env
        .as_deref()
        .and_then(find_workspace_root)
        .or_else(|| from_cwd.as_deref().and_then(find_workspace_root))
        .or_else(|| from_manifest.as_deref().and_then(find_workspace_root))
        .ok_or(RepoRootError)
}

fn find_workspace_root(start: &Path) -> Option<PathBuf> {
    let mut current = Some(start);
    while let Some(dir) = current {
        let cargo_toml = dir.join("Cargo.toml");
        if cargo_toml.is_file() && cargo_toml_declares_workspace(dir) {
            return Some(dir.to_path_buf());
        }
        current = dir.parent();
    }
    None
}

fn cargo_toml_declares_workspace(dir: &Path) -> bool {
    let Ok(dir_handle) = Dir::open_ambient_dir(dir, ambient_authority()) else {
        return false;
    };
    dir_handle
        .read_to_string("Cargo.toml")
        .ok()
        .is_some_and(|contents| contents.contains("[workspace]"))
}
