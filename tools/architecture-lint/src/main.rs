//! CLI entry point for the repo-local architecture lint.
//!
//! The main function establishes the capability boundary: it discovers the
//! workspace root using ambient filesystem access, then opens a capability
//! handle to the `backend/` directory which is passed to the lint library.

use std::fmt;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use architecture_lint::cargo_toml_declares_workspace;
use camino::Utf8Path;
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

/// Reads the process environment for the workspace-root override.
///
/// This is the binary's only ambient environment read. The search policy in
/// [`first_workspace_root`] takes its candidates as an argument, so it stays
/// testable without touching the process.
#[expect(
    clippy::disallowed_methods,
    reason = "composition root: the CLI reads CARGO_WORKSPACE_DIR once and \
              injects it into the search policy"
)]
fn workspace_dir_override() -> Option<PathBuf> {
    std::env::var("CARGO_WORKSPACE_DIR").ok().map(PathBuf::from)
}

/// Collects the ordered workspace-root candidates for the current process.
fn repo_root_candidates() -> Vec<PathBuf> {
    [
        workspace_dir_override(),
        std::env::current_dir().ok(),
        Some(PathBuf::from(env!("CARGO_MANIFEST_DIR"))),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn repo_root() -> Result<PathBuf, RepoRootError> {
    first_workspace_root(&repo_root_candidates())
}

/// Returns the first candidate whose directory (or an ancestor) declares a
/// workspace.
///
/// Taking the candidates explicitly keeps workspace discovery a pure search
/// over supplied paths, so tests exercise the precedence order without setting
/// `CARGO_WORKSPACE_DIR` on the process.
fn first_workspace_root(candidates: &[PathBuf]) -> Result<PathBuf, RepoRootError> {
    candidates
        .iter()
        .find_map(|candidate| find_workspace_root(candidate))
        .ok_or(RepoRootError)
}

fn find_workspace_root(start: &Path) -> Option<PathBuf> {
    let cargo_toml = Utf8Path::new("Cargo.toml");
    let mut current = Some(start);
    while let Some(dir) = current {
        let Ok(dir_handle) = Dir::open_ambient_dir(dir, ambient_authority()) else {
            current = dir.parent();
            continue;
        };
        if cargo_toml_declares_workspace(&dir_handle, cargo_toml) {
            return Some(dir.to_path_buf());
        }
        current = dir.parent();
    }
    None
}

#[cfg(test)]
mod tests {
    //! Unit tests for workspace-root discovery.

    use super::{PathBuf, first_workspace_root};

    /// Scenario: a candidate list whose first entry is not inside a workspace.
    ///
    /// Invariant: discovery skips candidates that resolve to no workspace and
    /// returns the first candidate that does, so precedence is decided by the
    /// supplied order rather than by the process environment.
    #[test]
    fn first_workspace_root_skips_candidates_without_a_workspace() {
        let sandbox = tempfile::tempdir().expect("tempdir");
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let candidates = vec![sandbox.path().to_path_buf(), manifest_dir];

        let root = first_workspace_root(&candidates).expect("the manifest dir has a workspace");

        assert!(
            root.join("Cargo.toml").is_file(),
            "the discovered root must contain the workspace manifest"
        );
    }

    /// Scenario: no candidate lies within a workspace.
    ///
    /// Invariant: discovery reports the dedicated error rather than falling
    /// back to an ambient path.
    #[test]
    fn first_workspace_root_reports_an_error_when_no_candidate_matches() {
        let sandbox = tempfile::tempdir().expect("tempdir");
        let candidates = vec![sandbox.path().to_path_buf()];

        assert!(
            first_workspace_root(&candidates).is_err(),
            "a candidate list with no workspace must not resolve to a root"
        );
    }
}
