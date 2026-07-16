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
//!
//! The stable-environment resolution and repair helpers live in the sibling
//! [`stable_cluster_env`] module so a dedicated unit-test target can exercise
//! them without compiling this cluster-handle acquisition path. Callers keep
//! importing `ensure_stable_cluster_environment` from here via the re-export
//! below.

use pg_embedded_setup_unpriv::{BootstrapResult, ClusterHandle};

#[path = "stable_cluster_env.rs"]
mod stable_cluster_env;

// Re-exported so existing `support::atexit_cleanup::ensure_stable_cluster_environment`
// call sites keep resolving after the pure helpers moved to `stable_cluster_env`.
pub(crate) use stable_cluster_env::ensure_stable_cluster_environment;

use stable_cluster_env::{SHARED_CLUSTER_RETRIES, SHARED_CLUSTER_RETRY_DELAY};

#[cfg(unix)]
mod exit_handler {
    //! Unix-only `libc::atexit` registration that stops the shared cluster's
    //! postmaster when the test binary exits.

    use std::path::PathBuf;
    use std::sync::OnceLock;
    use std::sync::atomic::{AtomicI32, Ordering};
    use std::time::Duration;

    use pg_embedded_setup_unpriv::ClusterHandle;

    use super::stable_cluster_env::unix_atexit::read_postmaster_pid;

    /// Postmaster PID captured at registration time.
    static PG_POSTMASTER_PID: AtomicI32 = AtomicI32::new(0);

    /// Data directory for re-reading `postmaster.pid` at exit time.
    static PG_DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

    /// Sends SIGTERM to the PostgreSQL postmaster and waits for shutdown.
    ///
    /// Registered via `libc::atexit` so the shared cluster is stopped when the
    /// test binary exits. Re-reads `postmaster.pid` at exit time and only signals
    /// when the on-disk PID still matches the stored value, guarding against PID
    /// reuse.
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
    pub(super) fn register_process_exit_cleanup(handle: &ClusterHandle) {
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
}

/// Returns the shared cluster handle and registers an atexit handler to stop
/// PostgreSQL when the test binary exits.
///
/// This is a thin wrapper around the library's `shared_cluster_handle()` that
/// adds cross-process cleanup for nextest compatibility.
///
/// # Environment preconditions
///
/// Callers **must** invoke [`ensure_stable_cluster_environment()`] before
/// calling this function. `shared_cluster_handle()` does not set up
/// `PG_PASSWORD` or `POSTGRESQL_RELEASES_URL` itself; separating setup from
/// access makes the command/query boundary explicit.
///
/// # Failure caching
///
/// `pg_embedded_setup_unpriv::test_support::shared_cluster_handle()` stores
/// its result in a library-internal `OnceLock`. Once a failure is recorded,
/// every subsequent call within the same process returns the same cached error
/// immediately — the retry loop below does **not** re-attempt the download.
/// The retries exist solely to handle the race window where a parallel test
/// thread's first-use bootstrap is still in progress. They provide no
/// protection against a failed bootstrap; for that, ensure the binary cache
/// is warm before the test process starts (see CI workflow and
/// `scripts/warm-pg-embedded-cache.sh`).
///
/// # Examples
///
/// ```rust,ignore
/// ensure_stable_cluster_environment();
/// let cluster = shared_cluster_handle()
///     .expect("embedded postgres cluster should be available");
/// let temp_db = cluster
///     .create_temporary_database()
///     .expect("temporary database should be created");
/// println!("connection URL: {}", temp_db.url());
/// ```
// Unit tests for this function are impractical: exercising the happy path
// requires a live embedded PostgreSQL cluster. Coverage is provided
// end-to-end by every BDD integration-test binary in the `pg-embed` nextest
// group. The internal helpers (`ensure_stable_cluster_environment`,
// `unix_atexit::read_postmaster_pid`, etc.) are unit-tested in the dedicated
// `atexit_cleanup_tests` integration-test target.
pub fn shared_cluster_handle() -> BootstrapResult<&'static ClusterHandle> {
    #[cfg(unix)]
    stable_cluster_env::unix_atexit::acquire_shared_cluster_process_lock()?;
    let mut attempt = 1;
    loop {
        match pg_embedded_setup_unpriv::test_support::shared_cluster_handle() {
            Ok(handle) => {
                #[cfg(unix)]
                exit_handler::register_process_exit_cleanup(handle);
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
