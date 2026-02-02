//! Shared capability-based filesystem helpers for example-data tests.

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::ambient_authority;
use cap_std::fs::Dir;
use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Create a unique temp file path under `target/example-data-tests`.
///
/// # Errors
///
/// Returns any filesystem errors encountered while creating the temp directory.
pub fn unique_temp_path(prefix: &str, file_name: &str) -> io::Result<Utf8PathBuf> {
    static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let process_id = std::process::id();
    let dir_name = format!("{prefix}-{process_id}-{counter}");
    let dir = Utf8PathBuf::from("target")
        .join("example-data-tests")
        .join(dir_name);
    let root = Dir::open_ambient_dir(".", ambient_authority())?;
    root.create_dir_all(&dir)?;
    Ok(dir.join(file_name))
}

/// Open the registry parent directory with a capability-based handle.
///
/// # Errors
///
/// Returns any filesystem errors encountered while opening the directory.
pub fn open_registry_dir(path: &Utf8Path) -> io::Result<Dir> {
    let parent = path.parent().unwrap_or_else(|| Utf8Path::new("."));
    Dir::open_ambient_dir(parent, ambient_authority())
}
