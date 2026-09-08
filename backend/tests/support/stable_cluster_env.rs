//! Stable password resolution and password-state repair for the shared
//! embedded PostgreSQL cluster.
//!
//! These helpers are the unit-testable half of the shared-cluster support:
//! they resolve the stable superuser password from an injected reader, repair
//! stale `.pgpass`/data-directory state, take the cross-process cluster lock,
//! and parse `postmaster.pid`. None of them writes to the process
//! environment. They are deliberately free of the
//! `libc::atexit` process-exit registration and cluster-handle acquisition
//! (see `atexit_cleanup.rs`), so the dedicated `atexit_cleanup_tests` target
//! can exercise them without compiling — and being forced to suppress — the
//! cluster machinery it never calls.

use std::time::Duration;

use pg_embedded_setup_unpriv::BootstrapResult;

#[cfg(unix)]
#[path = "password_state.rs"]
mod password_state;

#[cfg(unix)]
pub(crate) use password_state::PasswordStatePaths;

/// Repair paths on a platform with no `.pgpass` state to reconcile.
///
/// The platform difference lives in these leaf items rather than in
/// [`ensure_stable_cluster_environment_with`], so that function has one body
/// on every platform and needs no discarded binding to keep the compiler
/// quiet.
#[cfg(not(unix))]
pub(crate) struct PasswordStatePaths;

#[cfg(not(unix))]
impl PasswordStatePaths {
    /// Resolve the repair paths, which are empty off Unix.
    pub(crate) fn resolve(_read_env: impl Fn(&str) -> Option<String>) -> Self {
        Self
    }
}

pub(crate) const SHARED_CLUSTER_RETRIES: usize = 5;
pub(crate) const SHARED_CLUSTER_RETRY_DELAY: Duration = Duration::from_millis(500);

#[cfg(unix)]
pub(crate) mod unix_atexit {
    //! Unix-only shared-cluster process lock and `postmaster.pid` parsing.

    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    use std::sync::{Mutex, OnceLock};

    use color_eyre::eyre::eyre;
    use pg_embedded_setup_unpriv::{BootstrapError, BootstrapResult};

    const SHARED_CLUSTER_LOCK_FILE: &str = "wildside-pg-embedded-shared-cluster.lock";

    static SHARED_CLUSTER_PROCESS_LOCK_FD: OnceLock<i32> = OnceLock::new();
    static SHARED_CLUSTER_PROCESS_LOCK_INIT: Mutex<()> = Mutex::new(());

    /// Take the cross-process `flock` guarding the shared cluster directory.
    ///
    /// Idempotent within a process: once the descriptor is stored, later calls
    /// short-circuit rather than re-locking.
    pub(crate) fn acquire_shared_cluster_process_lock() -> BootstrapResult<()> {
        if SHARED_CLUSTER_PROCESS_LOCK_FD.get().is_some() {
            return Ok(());
        }

        let _init_guard = SHARED_CLUSTER_PROCESS_LOCK_INIT.lock().map_err(|error| {
            BootstrapError::from(eyre!(
                "acquire shared cluster process lock init mutex: {error}"
            ))
        })?;
        if SHARED_CLUSTER_PROCESS_LOCK_FD.get().is_some() {
            return Ok(());
        }

        let lock_path = std::env::temp_dir().join(SHARED_CLUSTER_LOCK_FILE);
        let lock_path_bytes = lock_path.as_os_str().as_bytes();
        let lock_path_cstring = CString::new(lock_path_bytes).map_err(|error| {
            BootstrapError::from(eyre!(
                "encode shared cluster lock path '{}': {error}",
                lock_path.display()
            ))
        })?;

        // SAFETY: `lock_path_cstring` is NUL-terminated and lives for the call.
        let fd = unsafe {
            libc::open(
                lock_path_cstring.as_ptr(),
                libc::O_CREAT | libc::O_RDWR,
                0o600,
            )
        };
        if fd < 0 {
            let error = std::io::Error::last_os_error();
            return Err(BootstrapError::from(eyre!(
                "open shared cluster lock file '{}': {error}",
                lock_path.display()
            )));
        }

        // SAFETY: `fd` is a valid descriptor from `open` above.
        let lock_result = unsafe { libc::flock(fd, libc::LOCK_EX) };
        if lock_result != 0 {
            let error = std::io::Error::last_os_error();
            // SAFETY: `fd` is valid and should be closed on lock failure.
            unsafe {
                libc::close(fd);
            }
            return Err(BootstrapError::from(eyre!(
                "acquire shared cluster lock '{}': {error}",
                lock_path.display()
            )));
        }

        let _ = SHARED_CLUSTER_PROCESS_LOCK_FD.set(fd);
        Ok(())
    }

    /// Reads the postmaster PID from the `postmaster.pid` file in `data_dir`.
    pub(crate) fn read_postmaster_pid(data_dir: &std::path::Path) -> Option<i32> {
        let dir =
            cap_std::fs::Dir::open_ambient_dir(data_dir, cap_std::ambient_authority()).ok()?;
        let content = dir.read_to_string("postmaster.pid").ok()?;
        content.lines().next()?.trim().parse().ok()
    }
}

/// The stable superuser password used when `PG_PASSWORD` is not already set.
pub(crate) const DEFAULT_PG_PASSWORD: &str = "wildside_embedded_test";

/// Resolves the superuser password the shared embedded cluster is expected to
/// use.
///
/// The resolution is pure: it returns an existing `PG_PASSWORD` unchanged and
/// otherwise yields [`DEFAULT_PG_PASSWORD`]. Nothing here writes to the
/// process. The value that the embedded PostgreSQL layer itself reads is
/// composed by the test runner — see the `[env]` table in
/// `.config/nextest.toml`, which supplies `PG_PASSWORD` and
/// `POSTGRESQL_RELEASES_URL` without overriding a value already present in the
/// parent environment.
///
/// `postgresql_embedded::Settings::default()` generates a random password on
/// each call. When the data directory already exists, `setup()` skips `initdb`,
/// leaving the cluster configured with the *original* password. Without a
/// stable value, subsequent nextest processes fail with `28P01 password
/// authentication failed`; resolving the same stable default here is what lets
/// the repair below detect a `.pgpass` written under a different one.
pub(crate) fn resolve_stable_password(read_env: impl Fn(&str) -> Option<String>) -> String {
    read_env("PG_PASSWORD")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_PG_PASSWORD.to_owned())
}

/// Reconciles stale embedded-cluster password state before the shared cluster
/// bootstraps.
///
/// Resolves the stable password and the repair paths through the injected
/// reader, then removes a `.pgpass` (and, when the data directory is the
/// default one, that directory) left behind by a run that used a different
/// password.
///
/// There is no ordering invariant against Tokio runtime construction any more:
/// this path performs no environment mutation, so it is safe to call at any
/// point in a test setup function.
pub(crate) fn ensure_stable_cluster_environment_with<R>(read_env: R) -> BootstrapResult<()>
where
    R: Fn(&str) -> Option<String>,
{
    let password = resolve_stable_password(&read_env);
    let paths = PasswordStatePaths::resolve(&read_env);
    // The repair path is fallible (lock acquisition and filesystem cleanup);
    // propagate any failure to the caller rather than hiding it behind a
    // deeper `.expect()`, so each setup boundary decides how to surface it.
    repair_password_state_serialized(password.as_bytes(), &paths)
}

/// Serializes `.pgpass`/data-directory repair so concurrent callers cannot race
/// on removing shared cluster state.
///
/// Two independent locks are required because the failure modes are distinct:
///
/// * **Cross-process** — concurrently starting nextest binaries share the
///   embedded cluster's data directory. `acquire_shared_cluster_process_lock`
///   takes the same `flock`-based process lock that `shared_cluster_handle`
///   uses (it is idempotent within a process, short-circuiting once the fd is
///   stored), so repair in one process cannot overlap repair in another.
/// * **Intra-process** — the `flock` guards distinct processes only; within a
///   single process the second caller short-circuits on the stored fd and never
///   re-locks. Two threads (for example under the threaded `cargo test` runner)
///   would then race on `.pgpass` removal, so a process-local mutex serializes
///   them as well.
///
/// Crate-visible so the concurrency tests can exercise the repair/locking
/// stage directly with a fixed password and sandboxed paths. This helper reads
/// only its arguments and never touches the process environment.
#[cfg(unix)]
pub(crate) fn repair_password_state_serialized(
    password: &[u8],
    paths: &PasswordStatePaths,
) -> BootstrapResult<()> {
    use color_eyre::eyre::eyre;
    static PASSWORD_STATE_REPAIR_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    unix_atexit::acquire_shared_cluster_process_lock()?;
    let _repair_guard = PASSWORD_STATE_REPAIR_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    repair_default_password_state(password, paths).map_err(|error| {
        pg_embedded_setup_unpriv::BootstrapError::from(eyre!(
            "repair shared cluster password state: {error}"
        ))
    })
}

/// Off Unix there is no embedded-cluster password state to repair, so the
/// resolved password and paths are accepted and ignored.
///
/// # Errors
///
/// Never fails; the signature matches the Unix arm so callers stay
/// platform-agnostic.
#[cfg(not(unix))]
pub(crate) fn repair_password_state_serialized(
    _password: &[u8],
    _paths: &PasswordStatePaths,
) -> BootstrapResult<()> {
    Ok(())
}

#[cfg(unix)]
fn repair_default_password_state(
    password: &[u8],
    paths: &PasswordStatePaths,
) -> std::io::Result<()> {
    password_state::repair_default_password_state(password, paths)
}
