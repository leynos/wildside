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
///
/// # Example
///
/// ```no_run
/// # use test_support::unique_temp_path;
/// # fn main() -> std::io::Result<()> {
/// let path = unique_temp_path("example", "seeds.json")?;
/// assert!(path.as_str().contains("example"));
/// # Ok(())
/// # }
/// ```
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

/// Create a unique temp path that does not create the directory.
///
/// This is useful for tests that need a missing registry path.
///
/// # Example
///
/// ```
/// # use test_support::unique_missing_path;
/// let path = unique_missing_path("missing.json");
/// assert!(path.ends_with("missing.json"));
/// ```
pub fn unique_missing_path(file_name: &str) -> Utf8PathBuf {
    static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir_name = format!("seed-registry-cli-missing-{counter}");
    Utf8PathBuf::from("target")
        .join("example-data-tests")
        .join(dir_name)
        .join(file_name)
}

/// Open the registry parent directory with a capability-based handle.
///
/// # Errors
///
/// Returns any filesystem errors encountered while opening the directory.
///
/// # Example
///
/// ```no_run
/// # use test_support::{open_registry_dir, unique_temp_path};
/// # fn main() -> std::io::Result<()> {
/// let path = unique_temp_path("example", "seeds.json")?;
/// let _dir = open_registry_dir(&path)?;
/// # Ok(())
/// # }
/// ```
pub fn open_registry_dir(path: &Utf8Path) -> io::Result<Dir> {
    let parent = path.parent().unwrap_or_else(|| Utf8Path::new("."));
    Dir::open_ambient_dir(parent, ambient_authority())
}

/// Remove the parent directory of a temp file path.
///
/// # Errors
///
/// Returns any filesystem errors encountered while removing the directory,
/// excluding `NotFound` which is treated as a successful cleanup.
///
/// # Example
///
/// ```no_run
/// # use test_support::{cleanup_path, unique_temp_path};
/// # fn main() -> std::io::Result<()> {
/// let path = unique_temp_path("example", "seeds.json")?;
/// cleanup_path(&path)?;
/// # Ok(())
/// # }
/// ```
pub fn cleanup_path(path: &Utf8Path) -> io::Result<()> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    let root = Dir::open_ambient_dir(".", ambient_authority())?;
    match root.remove_dir_all(parent) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    //! Exercises shared temp directory helpers.

    use super::*;

    #[test]
    fn temp_helpers_create_and_cleanup_paths() {
        let path =
            unique_temp_path("test-support", "seeds.json").expect("create temp registry path");
        let missing = unique_missing_path("missing.json");
        let dir = open_registry_dir(&path).expect("open registry dir");
        let file_name = path.file_name().expect("registry file name");
        dir.write(file_name, "{}").expect("write registry file");
        assert!(missing.ends_with("missing.json"));
        cleanup_path(&path).expect("cleanup temp dir");
    }
}
