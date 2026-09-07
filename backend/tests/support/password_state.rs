//! Password-file repair for embedded PostgreSQL integration tests.
//!
//! `postgresql_embedded` only writes the initdb password file when it does not
//! already exist. When the shared stable test password changes, a stale
//! `.pgpass` can initialize a fresh data directory with the old password while
//! clients connect with the new one.

use std::io;
use std::path::{Path, PathBuf};

use cap_std::ambient_authority;
use cap_std::fs::Dir;

pub(super) fn repair_default_password_state(
    password: &[u8],
    paths: &PasswordStatePaths,
) -> io::Result<()> {
    // A missing install directory means there is nothing to repair, so treat
    // it as a quiet success. Genuine failures (permissions, a path that is not
    // a directory, and so on) are propagated so the caller can surface them at
    // an explicit boundary rather than mistaking them for absent state.
    let Some(install_dir) = open_dir_if_exists(&paths.install_dir)? else {
        return Ok(());
    };

    // The data parent is opened lazily inside the removal branch: an absent
    // parent must still allow a stale `.pgpass` to be cleaned up rather than
    // short-circuiting the whole repair.
    repair_password_file_state(password, &install_dir, paths)
}

/// Open an ambient directory, treating only a missing path as a quiet no-op.
///
/// Returns `Ok(None)` when the directory does not exist, `Ok(Some(dir))` when
/// it opens successfully, and propagates every other I/O error (for example a
/// permission failure or a non-directory path) instead of hiding it as absent.
fn open_dir_if_exists(path: &Path) -> io::Result<Option<Dir>> {
    match Dir::open_ambient_dir(path, ambient_authority()) {
        Ok(dir) => Ok(Some(dir)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// Filesystem locations the stale-password repair operates on.
///
/// The paths are data, resolved once at a boundary and passed in. Tests build
/// the struct directly against a sandbox instead of overriding `PG_DATA_DIR`
/// and `PG_RUNTIME_DIR` on the process.
pub(crate) struct PasswordStatePaths {
    install_dir: PathBuf,
    data_parent: PathBuf,
    data_name: PathBuf,
    should_remove_data_dir: bool,
}

impl PasswordStatePaths {
    /// Resolve the repair paths through an injected environment reader.
    ///
    /// `PG_RUNTIME_DIR` and `PG_DATA_DIR` override the per-uid defaults under
    /// `/var/tmp`. Only the default data directory is eligible for deletion:
    /// an explicitly configured one belongs to the caller.
    pub(crate) fn resolve(read_env: impl Fn(&str) -> Option<String>) -> Self {
        // SAFETY: `geteuid` has no preconditions and does not modify memory.
        let uid = unsafe { libc::geteuid() };
        let base = PathBuf::from(format!("/var/tmp/pg-embed-{uid}"));
        let configured_data_dir = read_env("PG_DATA_DIR").map(PathBuf::from);
        let should_remove_data_dir = configured_data_dir.is_none();
        let data_dir = configured_data_dir.unwrap_or_else(|| base.join("data"));
        let install_dir = read_env("PG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| base.join("install"));

        let data_parent = data_dir
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_owned();
        let data_name = data_dir
            .file_name()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("data"));

        Self {
            install_dir,
            data_parent,
            data_name,
            should_remove_data_dir,
        }
    }
}

fn repair_password_file_state(
    password: &[u8],
    install_dir: &Dir,
    paths: &PasswordStatePaths,
) -> io::Result<()> {
    let existing_password = match install_dir.read(Path::new(".pgpass")) {
        Ok(contents) => contents,
        // No stale password file means nothing to repair.
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        // Permission or other read failures must not be mistaken for absence.
        Err(error) => return Err(error),
    };
    if existing_password == password {
        return Ok(());
    }

    // Clean the stale data directory before removing `.pgpass`. If the data
    // cleanup fails, this returns early with the error and leaves `.pgpass` in
    // place so a subsequent retry still detects the stale state; deleting
    // `.pgpass` first would let a retry short-circuit as "nothing to repair"
    // while the data directory stayed orphaned.
    if paths.should_remove_data_dir {
        // Open the data parent only when there is a default data directory to
        // delete. An absent parent means the directory never existed, so it is
        // a quiet no-op rather than a reason to skip the `.pgpass` cleanup
        // below.
        if let Some(data_parent) = open_dir_if_exists(&paths.data_parent)? {
            remove_dir_if_exists(&data_parent, &paths.data_name)?;
        }
    }

    install_dir.remove_file(Path::new(".pgpass"))?;

    Ok(())
}

fn remove_dir_if_exists(parent: &Dir, path: &Path) -> io::Result<()> {
    match parent.remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
#[path = "password_state_tests.rs"]
mod tests;
