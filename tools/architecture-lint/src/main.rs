//! CLI entry point for the repo-local architecture lint.

use std::io::{self, Write};
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
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("tools/architecture-lint must be two levels below repo root")
        .to_path_buf()
}
