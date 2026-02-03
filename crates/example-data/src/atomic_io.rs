//! Atomic file write operations.
//!
//! This module provides helpers for writing files atomically using a
//! temporary file and rename strategy, ensuring partial writes do not
//! corrupt the target file.

use std::io::{self, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use camino::{Utf8Component, Utf8Path};
use cap_std::fs::{Dir, OpenOptions};

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
/// ```ignore
/// use std::time::{SystemTime, UNIX_EPOCH};
/// // This example is illustrative; write_atomic is crate-private.
///
/// let suffix = SystemTime::now()
///     .duration_since(UNIX_EPOCH)
///     .map(|elapsed| elapsed.as_nanos())
///     .unwrap_or(0);
/// let dir_path = std::env::temp_dir().join(format!("example-data-{suffix}"));
/// std::fs::create_dir_all(&dir_path).expect("create temp dir");
/// let dir = cap_std::fs::Dir::open_ambient_dir(&dir_path, cap_std::ambient_authority())
///     .expect("open temp dir");
/// let path = camino::Utf8Path::new("registry.json");
///
/// write_atomic(&dir, path, r#"{ "version": 1 }"#).expect("write registry");
/// let contents = dir.read_to_string(path).expect("read registry");
///
/// assert!(contents.contains("\"version\""));
/// dir.remove_file(path).expect("clean up");
/// ```
pub(crate) fn write_atomic(
    dir: &Dir,
    path: &Utf8Path,
    contents: &str,
) -> Result<(), RegistryError> {
    let mut components = path.components();
    let (Some(Utf8Component::Normal(file_name)), None) = (components.next(), components.next())
    else {
        return Err(RegistryError::WriteError {
            path: path.to_path_buf(),
            message: "registry path must be a file".to_owned(),
        });
    };
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let tmp_name = format!(
        ".{}.tmp.{}.{}.{}",
        file_name,
        std::process::id(),
        suffix,
        counter
    );

    write_to_temp_file(dir, &tmp_name, path, contents)?;
    rename_temp_to_target(dir, &tmp_name, file_name, path)?;
    sync_parent_directory(dir);

    Ok(())
}

fn write_to_temp_file(
    dir: &Dir,
    tmp_name: &str,
    target_path: &Utf8Path,
    contents: &str,
) -> Result<(), RegistryError> {
    let tmp_path = target_path.with_file_name(tmp_name);
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    let mut file = dir
        .open_with(tmp_name, &options)
        .map_err(|err| RegistryError::WriteError {
            path: tmp_path.to_path_buf(),
            message: err.to_string(),
        })?;

    if let Err(err) = file.write_all(contents.as_bytes()) {
        drop(file);
        drop(dir.remove_file(tmp_name));
        return Err(RegistryError::WriteError {
            path: tmp_path.to_path_buf(),
            message: err.to_string(),
        });
    }

    if let Err(err) = file.sync_all() {
        drop(file);
        drop(dir.remove_file(tmp_name));
        return Err(RegistryError::WriteError {
            path: tmp_path.to_path_buf(),
            message: err.to_string(),
        });
    }

    Ok(())
}

fn rename_temp_to_target(
    dir: &Dir,
    tmp_name: &str,
    target_name: &str,
    target_path: &Utf8Path,
) -> Result<(), RegistryError> {
    if let Err(err) = rename_temp_to_target_impl(dir, tmp_name, target_name) {
        // Best-effort cleanup of temp file on rename failure.
        if dir.remove_file(tmp_name).is_err() {
            // Ignore cleanup failures.
        }
        return Err(RegistryError::WriteError {
            path: target_path.to_path_buf(),
            message: err.to_string(),
        });
    }
    Ok(())
}

#[cfg(windows)]
fn rename_temp_to_target_impl(dir: &Dir, tmp_name: &str, target_name: &str) -> io::Result<()> {
    // Windows rename fails if the target exists, so remove it first.
    match dir.remove_file(target_name) {
        Ok(()) => {}
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => return Err(err),
    }
    dir.rename(tmp_name, dir, target_name)
}

#[cfg(not(windows))]
fn rename_temp_to_target_impl(dir: &Dir, tmp_name: &str, target_name: &str) -> io::Result<()> {
    dir.rename(tmp_name, dir, target_name)
}

fn sync_parent_directory(parent: &Dir) {
    // Best-effort directory sync; ignore failures.
    if parent.open(".").and_then(|dir| dir.sync_all()).is_err() {
        // Ignore sync failures.
    }
}
