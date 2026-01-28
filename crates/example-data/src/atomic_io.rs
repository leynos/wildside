//! Atomic file write operations.
//!
//! This module provides helpers for writing files atomically using a
//! temporary file and rename strategy, ensuring partial writes do not
//! corrupt the target file.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::RegistryError;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Writes contents to a file atomically using a temp file and rename.
///
/// The function writes to a hidden temporary file in the same directory,
/// then renames it to the target path. This ensures the target file is
/// never partially written.
///
/// # Errors
///
/// Returns [`RegistryError::WriteError`] if the file cannot be written.
///
/// # Example
///
/// ```no_run
/// use std::time::{SystemTime, UNIX_EPOCH};
///
/// use crate::atomic_io::write_atomic;
///
/// let suffix = SystemTime::now()
///     .duration_since(UNIX_EPOCH)
///     .map(|elapsed| elapsed.as_nanos())
///     .unwrap_or(0);
/// let dir = std::env::temp_dir().join(format!("example-data-{suffix}"));
/// std::fs::create_dir_all(&dir).expect("create temp dir");
/// let path = dir.join("registry.json");
///
/// write_atomic(&path, r#"{ "version": 1 }"#).expect("write registry");
/// let contents = std::fs::read_to_string(&path).expect("read registry");
///
/// assert!(contents.contains("\"version\""));
/// std::fs::remove_file(&path).expect("clean up");
/// ```
pub(crate) fn write_atomic(path: &Path, contents: &str) -> Result<(), RegistryError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path.file_name().ok_or_else(|| RegistryError::WriteError {
        path: path.to_path_buf(),
        message: "registry path must be a file".to_owned(),
    })?;
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or(0);
    let tmp_name = format!(
        ".{}.tmp.{}.{}.{}",
        file_name.to_string_lossy(),
        std::process::id(),
        suffix,
        counter
    );
    let tmp_path = parent.join(tmp_name);

    write_to_temp_file(&tmp_path, contents)?;
    rename_temp_to_target(&tmp_path, path)?;
    sync_parent_directory(parent);

    Ok(())
}

fn write_to_temp_file(tmp_path: &Path, contents: &str) -> Result<(), RegistryError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(tmp_path)
        .map_err(|e| RegistryError::WriteError {
            path: tmp_path.to_path_buf(),
            message: e.to_string(),
        })?;

    file.write_all(contents.as_bytes())
        .map_err(|e| RegistryError::WriteError {
            path: tmp_path.to_path_buf(),
            message: e.to_string(),
        })?;

    file.sync_all().map_err(|e| RegistryError::WriteError {
        path: tmp_path.to_path_buf(),
        message: e.to_string(),
    })?;

    Ok(())
}

fn rename_temp_to_target(tmp_path: &Path, target: &Path) -> Result<(), RegistryError> {
    if let Err(err) = rename_temp_to_target_impl(tmp_path, target) {
        // Best-effort cleanup of temp file on rename failure.
        if fs::remove_file(tmp_path).is_err() {
            // Ignore cleanup failures.
        }
        return Err(RegistryError::WriteError {
            path: target.to_path_buf(),
            message: err.to_string(),
        });
    }
    Ok(())
}

#[cfg(windows)]
fn rename_temp_to_target_impl(tmp_path: &Path, target: &Path) -> io::Result<()> {
    // Windows rename fails if the target exists, so remove it first.
    if target.exists() {
        fs::remove_file(target)?;
    }
    fs::rename(tmp_path, target)
}

#[cfg(not(windows))]
fn rename_temp_to_target_impl(tmp_path: &Path, target: &Path) -> io::Result<()> {
    fs::rename(tmp_path, target)
}

fn sync_parent_directory(parent: &Path) {
    // Best-effort directory sync; ignore failures.
    if File::open(parent).and_then(|dir| dir.sync_all()).is_err() {
        // Ignore sync failures.
    }
}
