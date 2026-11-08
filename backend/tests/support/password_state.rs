//! Retained-state repair for embedded PostgreSQL integration tests.
//!
//! `postgresql_embedded` only writes the initdb password file when it does not
//! already exist. When the shared stable test password changes, a stale
//! `.pgpass` can initialize a fresh data directory with the old password while
//! clients connect with the new one.
//!
//! The same directory is also retained across a change to `PG_VERSION_REQ`,
//! and across a bootstrap that never finished. PostgreSQL refuses to start when
//! the stored `PG_VERSION` names a different major version from the server
//! binary, and `initdb` refuses a directory that is not empty. A default data
//! directory carrying either fault is removed before the cluster bootstraps.

use std::io;
use std::path::{Path, PathBuf};

use cap_std::ambient_authority;
use cap_std::fs::Dir;

/// Remove embedded-cluster password state left behind under a different
/// password.
///
/// A missing install directory is a quiet success; every other I/O failure is
/// propagated so the caller can surface it.
pub(super) fn repair_default_password_state(
    password: &[u8],
    paths: &PasswordStatePaths,
) -> io::Result<()> {
    // A data directory the next bootstrap cannot reuse fails before any
    // password is checked, so it is repaired first and independently of the
    // install directory's presence.
    repair_unusable_data_directory(paths)?;

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
    expected_major_version: Option<u32>,
}

impl PasswordStatePaths {
    /// Resolve the repair paths through an injected environment reader.
    ///
    /// `PG_RUNTIME_DIR` and `PG_DATA_DIR` override the per-uid defaults under
    /// `/var/tmp`. Only the default data directory is eligible for deletion:
    /// an explicitly configured one belongs to the caller. An exact
    /// `PG_VERSION_REQ` pin supplies the major version a retained data
    /// directory must match; a range requirement supplies none, because more
    /// than one stored major satisfies it.
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
            expected_major_version: read_env("PG_VERSION_REQ")
                .as_deref()
                .and_then(major_version),
        }
    }
}

/// Extract the major version from an exact pin or a `PG_VERSION` file.
///
/// PostgreSQL writes a bare number into `PG_VERSION`, and a bare number is
/// also an exact `PG_VERSION_REQ`. A `=` requirement such as `=16.10.0` pins
/// the same way. Every other operator denotes a range: `>=17` is satisfied by
/// an 18 directory, so treating it as a pin to 17 would delete a directory the
/// requirement still accepts. Ranges therefore yield `None`, which leaves a
/// retained directory in place rather than guessing at a mismatch.
fn major_version(text: &str) -> Option<u32> {
    let trimmed = text.trim();
    let is_exact = trimmed
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_digit() || first == '=');
    if !is_exact {
        return None;
    }
    let digits: String = trimmed
        .trim_start_matches(|character: char| !character.is_ascii_digit())
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

/// Report whether `path` exists inside `dir`, propagating real I/O failures.
fn path_exists(dir: &Dir, path: &Path) -> io::Result<bool> {
    match dir.metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

/// Report whether a running postmaster holds the data directory.
///
/// `/var/tmp/pg-embed-<uid>` is shared by every project under the same user,
/// and another project's cluster does not take Wildside's process lock, so the
/// lock alone cannot prove the directory is ours. A `postmaster.pid` naming a
/// live process is the only ownership evidence available at this boundary; a
/// stale pid file left by an exited cluster does not block removal, because
/// that is exactly the state the repair exists to clean up.
fn data_directory_is_in_use(data_parent: &Dir, data_name: &Path) -> io::Result<bool> {
    let contents = match data_parent.read_to_string(data_name.join("postmaster.pid")) {
        Ok(contents) => contents,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    let Some(pid) = contents
        .lines()
        .next()
        .and_then(|line| line.trim().parse::<i32>().ok())
    else {
        return Ok(false);
    };
    if pid <= 0 {
        return Ok(false);
    }
    // SAFETY: signal 0 performs the existence and permission check only; it
    // delivers no signal and has no other side effect.
    if unsafe { libc::kill(pid, 0) } == 0 {
        return Ok(true);
    }
    // `EPERM` means the process exists but belongs to another user, which is
    // still a live holder of the directory.
    Ok(io::Error::last_os_error().kind() == io::ErrorKind::PermissionDenied)
}

/// Remove the default data directory when the next bootstrap cannot reuse it.
///
/// `postgresql_embedded` decides whether to run `initdb` by looking for
/// `<data dir>/postgresql.conf` alone, and `initdb` then refuses a directory
/// that is not empty. Two retained states therefore block every later
/// bootstrap until the directory is gone:
///
/// * a directory left by a bootstrap that never finished, which has files but
///   no `postgresql.conf`, so the library runs `initdb` over it; and
/// * a directory initialized by another major version, whose stored
///   `PG_VERSION` disagrees with `PG_VERSION_REQ`, so PostgreSQL refuses to
///   start.
///
/// Removing the default directory lets the next bootstrap initialize a
/// matching one. An explicitly configured `PG_DATA_DIR` belongs to the caller
/// and is never removed, an absent directory is a quiet no-op, and a directory
/// held by a running cluster is left alone because it may belong to another
/// project sharing the per-user default.
fn repair_unusable_data_directory(paths: &PasswordStatePaths) -> io::Result<()> {
    if !paths.should_remove_data_dir {
        return Ok(());
    }
    let Some(data_parent) = open_dir_if_exists(&paths.data_parent)? else {
        return Ok(());
    };
    let data_name = &paths.data_name;

    // The default data directory is shared with every other project under the
    // same user, and those clusters do not hold Wildside's lock. A running one
    // is not ours to repair or delete, so leave its data alone.
    if data_directory_is_in_use(&data_parent, data_name)? {
        return Ok(());
    }

    if !path_exists(&data_parent, &data_name.join("postgresql.conf"))? {
        return remove_dir_if_exists(&data_parent, data_name);
    }
    let Some(expected) = paths.expected_major_version else {
        return Ok(());
    };
    let stored = match data_parent.read_to_string(data_name.join("PG_VERSION")) {
        Ok(stored) => stored,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if major_version(&stored) == Some(expected) {
        return Ok(());
    }
    remove_dir_if_exists(&data_parent, data_name)
}

/// Compare the stored `.pgpass` with `password` and clear the stale state.
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
            // The shared directory may belong to another project's running
            // cluster; removing it would disrupt that project's tests.
            if !data_directory_is_in_use(&data_parent, &paths.data_name)? {
                remove_dir_if_exists(&data_parent, &paths.data_name)?;
            }
        }
    }

    install_dir.remove_file(Path::new(".pgpass"))?;

    Ok(())
}

/// Remove a directory tree, treating an absent path as a quiet success.
fn remove_dir_if_exists(parent: &Dir, path: &Path) -> io::Result<()> {
    match parent.remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
#[path = "password_state_fixture.rs"]
mod fixture;

#[cfg(test)]
#[path = "password_state_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "password_state_paths_tests.rs"]
mod paths_tests;
