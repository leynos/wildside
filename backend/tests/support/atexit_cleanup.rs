//! Process-exit cleanup for the shared embedded PostgreSQL cluster.
//!
//! `pg-embed-setup-unpriv`'s [`shared_cluster_handle()`] intentionally leaks
//! the [`ClusterGuard`] so the cluster persists for the process lifetime. In a
//! single-binary test runner this is fine — the OS reclaims everything at exit.
//! Under `nextest`, each test binary is a separate process, and a still-running
//! PostgreSQL blocks subsequent binaries from bootstrapping on the same data
//! directory.
//!
//! This module registers a `libc::atexit` handler that reads `postmaster.pid`,
//! sends `SIGTERM`, and waits for graceful shutdown, bridging the gap until the
//! library provides built-in process-exit shutdown.

#[cfg(unix)]
use std::ffi::CString;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::path::PathBuf;
#[cfg(unix)]
use std::sync::OnceLock;
#[cfg(unix)]
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Duration;

#[cfg(unix)]
use color_eyre::eyre::eyre;
#[cfg(unix)]
use pg_embedded_setup_unpriv::BootstrapError;
use pg_embedded_setup_unpriv::{BootstrapResult, ClusterHandle};

const SHARED_CLUSTER_RETRIES: usize = 5;
const SHARED_CLUSTER_RETRY_DELAY: Duration = Duration::from_millis(500);
#[cfg(unix)]
const SHARED_CLUSTER_LOCK_FILE: &str = "wildside-pg-embedded-shared-cluster.lock";

/// Postmaster PID captured at registration time.
#[cfg(unix)]
static PG_POSTMASTER_PID: AtomicI32 = AtomicI32::new(0);

/// Data directory for re-reading `postmaster.pid` at exit time.
#[cfg(unix)]
static PG_DATA_DIR: OnceLock<PathBuf> = OnceLock::new();
#[cfg(unix)]
static SHARED_CLUSTER_PROCESS_LOCK_FD: OnceLock<i32> = OnceLock::new();

#[cfg(unix)]
fn acquire_shared_cluster_process_lock() -> BootstrapResult<()> {
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

    if SHARED_CLUSTER_PROCESS_LOCK_FD.set(fd).is_err() {
        // SAFETY: `fd` is valid and must be closed when another caller won `set`.
        unsafe {
            libc::close(fd);
        }
    }
    Ok(())
}

/// Returns the shared cluster handle and registers an atexit handler to stop
/// PostgreSQL when the test binary exits.
///
/// This is a thin wrapper around the library's `shared_cluster_handle()` that
/// adds cross-process cleanup for nextest compatibility.
///
/// # Examples
///
/// ```rust,ignore
/// let cluster = shared_cluster_handle()
///     .expect("embedded postgres cluster should be available");
/// let temp_db = cluster
///     .create_temporary_database()
///     .expect("temporary database should be created");
/// println!("connection URL: {}", temp_db.url());
/// ```
pub fn shared_cluster_handle() -> BootstrapResult<&'static ClusterHandle> {
    ensure_stable_password();
    #[cfg(unix)]
    acquire_shared_cluster_process_lock()?;
    let mut attempt = 1;
    loop {
        match pg_embedded_setup_unpriv::test_support::shared_cluster_handle() {
            Ok(handle) => {
                #[cfg(unix)]
                register_process_exit_cleanup(handle);
                return Ok(handle);
            }
            Err(error) => {
                if attempt >= SHARED_CLUSTER_RETRIES {
                    return Err(error);
                }
                std::thread::sleep(SHARED_CLUSTER_RETRY_DELAY);
                attempt += 1;
            }
        }
    }
}

/// Ensures `PG_PASSWORD` is set to a stable value so the password remains
/// consistent across process invocations that reuse the same data directory.
///
/// `postgresql_embedded::Settings::default()` generates a random password on
/// each call. When the data directory already exists, `setup()` skips `initdb`,
/// leaving the cluster configured with the *original* password. Without a
/// stable override, subsequent nextest processes fail with `28P01 password
/// authentication failed`.
fn ensure_stable_password() {
    if std::env::var_os("PG_PASSWORD").is_none() {
        // SAFETY: called before the library spawns any threads. The shared
        // cluster singleton serializes access with a `Mutex`, so this runs at
        // most once per process.
        unsafe {
            std::env::set_var("PG_PASSWORD", "wildside_embedded_test");
        }
    }
}

/// Reads the postmaster PID from the `postmaster.pid` file in `data_dir`.
#[cfg(unix)]
fn read_postmaster_pid(data_dir: &std::path::Path) -> Option<i32> {
    let dir = cap_std::fs::Dir::open_ambient_dir(data_dir, cap_std::ambient_authority()).ok()?;
    let content = dir.read_to_string("postmaster.pid").ok()?;
    content.lines().next()?.trim().parse().ok()
}

/// Sends SIGTERM to the PostgreSQL postmaster and waits for shutdown.
///
/// Registered via `libc::atexit` so the shared cluster is stopped when the
/// test binary exits. Re-reads `postmaster.pid` at exit time and only signals
/// when the on-disk PID still matches the stored value, guarding against PID
/// reuse.
#[cfg(unix)]
extern "C" fn stop_postgres_on_exit() {
    let stored_pid = PG_POSTMASTER_PID.load(Ordering::Relaxed);
    if stored_pid <= 0 {
        return;
    }

    // Re-read postmaster.pid to guard against PID reuse.
    let pid = match PG_DATA_DIR.get().and_then(|dir| read_postmaster_pid(dir)) {
        Some(current_pid) if current_pid == stored_pid => current_pid,
        _ => return,
    };

    // SAFETY: `pid` was validated against the on-disk `postmaster.pid`.
    // SIGTERM triggers a graceful "smart shutdown"; signal 0 probes liveness.
    unsafe {
        if libc::kill(pid, libc::SIGTERM) != 0 {
            return;
        }
    }

    // Wait up to five seconds for PostgreSQL to exit gracefully.
    for _ in 0..50 {
        std::thread::sleep(Duration::from_millis(100));
        // SAFETY: signal 0 checks whether the process still exists.
        if unsafe { libc::kill(pid, 0) } != 0 {
            return;
        }
    }

    // SAFETY: force-kill after the graceful shutdown budget expires.
    unsafe {
        libc::kill(pid, libc::SIGKILL);
    }
}

/// Records the postmaster PID and registers an `atexit` handler so the
/// shared cluster is stopped when the test binary exits. Uses
/// `compare_exchange` to ensure the handler is registered at most once.
#[cfg(unix)]
fn register_process_exit_cleanup(handle: &ClusterHandle) {
    let data_dir = &handle.settings().data_dir;
    let Some(pid) = read_postmaster_pid(data_dir) else {
        return;
    };

    // Only register once: if PG_POSTMASTER_PID is still 0, swap in the real
    // PID. If it was already set, another call got here first.
    if PG_POSTMASTER_PID
        .compare_exchange(0, pid, Ordering::Relaxed, Ordering::Relaxed)
        .is_err()
    {
        return;
    }

    let _ = PG_DATA_DIR.set(data_dir.clone());

    // SAFETY: `stop_postgres_on_exit` is a valid `extern "C"` function with
    // no preconditions beyond the atomic PID being set (done above).
    let rc = unsafe { libc::atexit(stop_postgres_on_exit) };
    if rc != 0 {
        eprintln!(
            concat!(
                "pg-embed: failed to register atexit handler (rc={rc}); ",
                "PostgreSQL process (PID {pid}) may outlive the test binary"
            ),
            rc = rc,
            pid = pid
        );
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for atexit cleanup helpers.

    use cap_std::ambient_authority;
    use cap_std::fs::Dir;

    #[cfg(unix)]
    fn write_postmaster_pid(dir_path: &std::path::Path, content: &str) {
        let dir = Dir::open_ambient_dir(dir_path, ambient_authority()).expect("open dir");
        dir.write("postmaster.pid", content).expect("write");
    }

    #[cfg(unix)]
    #[test]
    fn read_postmaster_pid_parses_first_line() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_postmaster_pid(dir.path(), "12345\n/some/path\n5432\n");
        assert_eq!(super::read_postmaster_pid(dir.path()), Some(12345));
    }

    #[cfg(unix)]
    #[test]
    fn read_postmaster_pid_returns_none_for_missing_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert_eq!(super::read_postmaster_pid(dir.path()), None);
    }

    #[cfg(unix)]
    #[test]
    fn read_postmaster_pid_returns_none_for_non_numeric_content() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_postmaster_pid(dir.path(), "not-a-number\n");
        assert_eq!(super::read_postmaster_pid(dir.path()), None);
    }

    #[test]
    fn ensure_stable_password_does_not_overwrite_existing_value() {
        let _guard = env_lock::lock_env([("PG_PASSWORD", Some("custom_value"))]);
        super::ensure_stable_password();
        assert_eq!(
            std::env::var("PG_PASSWORD").expect("PG_PASSWORD should be set"),
            "custom_value",
            "ensure_stable_password should not overwrite an existing PG_PASSWORD"
        );
    }
}
