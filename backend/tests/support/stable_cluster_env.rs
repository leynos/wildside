//! Stable process-environment resolution and repair for the shared embedded
//! PostgreSQL cluster.
//!
//! These helpers are the unit-testable half of the shared-cluster support:
//! they resolve `PG_PASSWORD`/`POSTGRESQL_RELEASES_URL` to stable values,
//! repair stale `.pgpass`/data-directory state, take the cross-process cluster
//! lock, and parse `postmaster.pid`. They are deliberately free of the
//! `libc::atexit` process-exit registration and cluster-handle acquisition
//! (see `atexit_cleanup.rs`), so the dedicated `atexit_cleanup_tests` target
//! can exercise them without compiling — and being forced to suppress — the
//! cluster machinery it never calls.

use std::time::Duration;

use pg_embedded_setup_unpriv::BootstrapResult;

#[cfg(unix)]
#[path = "password_state.rs"]
mod password_state;

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

/// Caches the resolved `PG_PASSWORD` so the environment is reconciled exactly
/// once per process, no matter how many callers (or threads) invoke
/// [`ensure_stable_cluster_environment`].
static STABLE_ENV_INIT: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// Ensures that `PG_PASSWORD` and `POSTGRESQL_RELEASES_URL` are both set to
/// stable values before the shared embedded cluster is initialized.
///
/// Both variables are resolved exactly once per process inside a
/// [`OnceLock`](std::sync::OnceLock), so the `std::env::set_var` calls run at
/// most once regardless of caller concurrency and concurrent callers within a
/// single test binary cannot race on the environment. The resolution itself
/// lives in `resolve_stable_env` so tests can exercise the first-call logic
/// directly, separately from the cached-reuse path.
///
/// # Ordering invariant
///
/// On its first call this helper may invoke `std::env::set_var`, which is
/// undefined behaviour once other threads exist. Every test setup path **must**
/// call `ensure_stable_cluster_environment` before constructing a Tokio runtime
/// (`Runtime::new` spawns worker threads). Concretely, place this call above the
/// first runtime construction in each setup function; do not defer it until
/// after a runtime — or a runtime-owning fixture — has been created.
///
/// `postgresql_embedded::Settings::default()` generates a random password on
/// each call. When the data directory already exists, `setup()` skips `initdb`,
/// leaving the cluster configured with the *original* password. Without a
/// stable override, subsequent nextest processes fail with `28P01 password
/// authentication failed`.
///
/// `POSTGRESQL_RELEASES_URL` is pinned to the Theseus binaries mirror so that
/// the binary download source remains stable across crate upgrades and is not
/// subject to transient GitHub Releases fetch failures (misreported by reqwest
/// as "error decoding response body").
///
/// After the environment is resolved, `ensure_stable_cluster_environment` calls
/// `repair_password_state_serialized` so stale embedded-cluster password files
/// and default data directories are reconciled before initialization. That
/// keeps the cluster aligned with the stable `PG_PASSWORD` override and prevents
/// leftover authentication state from earlier runs.
pub(crate) fn ensure_stable_cluster_environment() -> BootstrapResult<()> {
    // Borrow the cached password directly; no clone is needed because the
    // repair helper only reads the bytes.
    let password = STABLE_ENV_INIT.get_or_init(resolve_stable_env);

    // The repair path is fallible (lock acquisition and filesystem cleanup);
    // propagate any failure to the caller rather than hiding it behind a
    // deeper `.expect()`, so fallibility is part of the helper's signature and
    // each setup boundary decides how to surface it.
    repair_password_state_serialized(password.as_bytes())
}

/// Resolves `PG_PASSWORD` and `POSTGRESQL_RELEASES_URL` to their stable values,
/// applying the defaults when either is unset, and returns the resolved
/// password.
///
/// This is the first-call body cached by `STABLE_ENV_INIT`. Tests call it
/// directly to cover the resolution logic in isolation from the process-wide
/// `OnceLock` caching.
///
/// # Safety of the `set_var` calls
///
/// `std::env::set_var` is not thread-safe, so callers must hold exclusive
/// access to the environment: production reaches this function through
/// `STABLE_ENV_INIT.get_or_init`, which runs it at most once per process, and
/// tests hold the `env_lock` mutex around their calls.
pub(crate) fn resolve_stable_env() -> String {
    let value = std::env::var("PG_PASSWORD").unwrap_or_else(|_| {
        let value = "wildside_embedded_test".to_owned();
        // SAFETY: the caller holds exclusive environment access; see the
        // function's Safety section.
        unsafe {
            std::env::set_var("PG_PASSWORD", value.as_str());
        }
        value
    });

    if std::env::var_os("POSTGRESQL_RELEASES_URL").is_none() {
        // Pin to Theseus binaries to avoid transient fetch failures in CI that
        // reqwest misreports as "error decoding response body".
        // SAFETY: the caller holds exclusive environment access; see above.
        unsafe {
            std::env::set_var(
                "POSTGRESQL_RELEASES_URL",
                "https://github.com/theseus-rs/postgresql-binaries",
            );
        }
    }

    value
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
#[cfg(unix)]
fn repair_password_state_serialized(password: &[u8]) -> BootstrapResult<()> {
    use color_eyre::eyre::eyre;
    static PASSWORD_STATE_REPAIR_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    unix_atexit::acquire_shared_cluster_process_lock()?;
    let _repair_guard = PASSWORD_STATE_REPAIR_LOCK
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    repair_default_password_state(password).map_err(|error| {
        pg_embedded_setup_unpriv::BootstrapError::from(eyre!(
            "repair shared cluster password state: {error}"
        ))
    })
}

#[cfg(not(unix))]
fn repair_password_state_serialized(_password: &[u8]) -> BootstrapResult<()> {
    Ok(())
}

#[cfg(unix)]
fn repair_default_password_state(password: &[u8]) -> std::io::Result<()> {
    password_state::repair_default_password_state(password)
}
