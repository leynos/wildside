//! Helpers for bootstrapping embedded PostgreSQL in integration tests.
//!
//! `pg-embed-setup-unpriv` defaults to using `/var/tmp` for installation and
//! data directories. Under the Codex CLI sandbox, writing outside of the
//! workspace is blocked, so tests that rely on the embedded cluster need to
//! override these paths.
//!
//! This module scopes `PG_RUNTIME_DIR` and `PG_DATA_DIR` overrides to the
//! bootstrap call and serializes environment mutation to avoid global
//! environment races across parallel tests.
//!
//! When either `PG_RUNTIME_DIR` or `PG_DATA_DIR` is missing, this module sets
//! both for the duration of the bootstrap, ensuring the embedded cluster uses
//! a consistent workspace-backed configuration.

use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use cap_std::{ambient_authority, fs::Dir};
use env_lock::lock_env;
use pg_embedded_setup_unpriv::{ClusterHandle, TestCluster};

static PG_EMBED_BOOTSTRAP_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
static SHARED_CLUSTER_HANDLE: OnceLock<ClusterHandle> = OnceLock::new();

/// PostgreSQL postmaster PID for process-exit cleanup.
///
/// Populated once the shared cluster starts; read by the `atexit` handler to
/// send SIGTERM so orphaned PostgreSQL processes do not survive the test binary.
/// This is a temporary workaround until `pg-embed-setup-unpriv` provides a
/// built-in process-exit shutdown mechanism.
#[cfg(unix)]
static PG_POSTMASTER_PID: AtomicI32 = AtomicI32::new(0);

/// Data directory for the shared cluster, used by the `atexit` handler to
/// re-read `postmaster.pid` and guard against PID reuse.
#[cfg(unix)]
static PG_DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Maximum number of retry attempts for transient network errors.
const MAX_RETRIES: u32 = 3;

/// Base delay between retry attempts (doubles with each retry).
const RETRY_DELAY_MS: u64 = 500;

fn pg_embed_target_dir() -> PathBuf {
    if let Some(target_dir) = std::env::var_os("CARGO_TARGET_DIR") {
        return PathBuf::from(target_dir).join("pg-embed");
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("target")
        .join("pg-embed")
}

fn create_shared_pg_embed_dirs() -> Result<(PathBuf, PathBuf), std::io::Error> {
    let base = pg_embed_target_dir().join(format!("shared-{}", std::process::id()));
    let runtime_dir = base.join("install");
    let data_dir = base.join("data");

    Dir::create_ambient_dir_all(&runtime_dir, ambient_authority())?;
    Dir::create_ambient_dir_all(&data_dir, ambient_authority())?;

    Ok((runtime_dir, data_dir))
}

/// Returns true if the error message suggests a transient network issue.
fn is_transient_error(err: &str) -> bool {
    let transient_patterns = [
        "error decoding response body",
        "connection reset",
        "connection refused",
        "timeout",
        "timed out",
        "temporarily unavailable",
        "network unreachable",
        "dns error",
        "failed to lookup",
        "failed to connect to admin database",
    ];

    let err_lower = err.to_lowercase();
    transient_patterns
        .iter()
        .any(|pattern| err_lower.contains(pattern))
}

fn bootstrap_with_retries<T>(
    mut bootstrap: impl FnMut() -> Result<T, String>,
) -> Result<T, String> {
    let mut last_error = String::new();
    for attempt in 0..=MAX_RETRIES {
        match bootstrap() {
            Ok(value) => return Ok(value),
            Err(err) => {
                last_error = err;
                if attempt < MAX_RETRIES && is_transient_error(&last_error) {
                    let delay = Duration::from_millis(RETRY_DELAY_MS * (1 << attempt));
                    eprintln!(
                        "pg-embed: transient error on attempt {}/{}, retrying in {:?}: {last_error}",
                        attempt + 1,
                        MAX_RETRIES + 1,
                        delay
                    );
                    std::thread::sleep(delay);
                } else {
                    break;
                }
            }
        }
    }

    Err(last_error)
}

/// Reads the PostgreSQL postmaster PID from the data directory.
#[cfg(unix)]
fn read_postmaster_pid(data_dir: &Path) -> Option<i32> {
    let dir = Dir::open_ambient_dir(data_dir, ambient_authority()).ok()?;
    let content = dir.read_to_string("postmaster.pid").ok()?;
    content.lines().next()?.trim().parse().ok()
}

/// Sends SIGTERM to the PostgreSQL postmaster and waits for shutdown.
///
/// Registered via `libc::atexit` so the shared cluster is stopped when the
/// test binary exits, even though [`ClusterGuard`] is deliberately forgotten
/// (because it is `!Send` and cannot be stored in `OnceLock`). Re-reads
/// `postmaster.pid` at exit time and only signals when the on-disk PID still
/// matches the stored value, guarding against PID reuse.
#[cfg(unix)]
extern "C" fn stop_postgres_on_exit() {
    let stored_pid = PG_POSTMASTER_PID.load(Ordering::Relaxed);
    if stored_pid <= 0 {
        return;
    }

    // Re-read postmaster.pid to guard against PID reuse: if PostgreSQL
    // exited early and the OS recycled the PID, the file will be absent
    // or contain a different value.
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
            "pg-embed: failed to register atexit handler (rc={rc}); \
             PostgreSQL process (PID {pid}) may outlive the test binary"
        );
    }
}

fn init_shared_cluster_handle() -> Result<&'static ClusterHandle, String> {
    if let Some(handle) = SHARED_CLUSTER_HANDLE.get() {
        return Ok(handle);
    }

    let (handle, guard) = TestCluster::new_split().map_err(|err| format!("{err:?}"))?;
    handle
        .database_exists("postgres")
        .map_err(|err| format!("{err:?}"))?;

    // `ClusterHandle` is cheap to retain and does not own teardown behaviour;
    // cleanup is bound to `guard`. Forgetting the guard is intentional here so
    // shared-cluster ownership remains process-lifetime for test binaries.
    // The `atexit` handler compensates by stopping PostgreSQL on process exit.
    std::mem::forget(guard);
    #[cfg(unix)]
    register_process_exit_cleanup(&handle);
    let _ = SHARED_CLUSTER_HANDLE.set(handle);

    SHARED_CLUSTER_HANDLE
        .get()
        .ok_or_else(|| "shared cluster handle was not retained after initialization".to_owned())
}

fn are_env_dirs_ready() -> bool {
    let runtime_dir_env = std::env::var_os("PG_RUNTIME_DIR");
    let data_dir_env = std::env::var_os("PG_DATA_DIR");

    match (&runtime_dir_env, &data_dir_env) {
        (Some(runtime_dir), Some(data_dir)) => {
            ensure_dir(Path::new(runtime_dir)) && ensure_dir(Path::new(data_dir))
        }
        _ => false,
    }
}

fn build_env_overrides(
    should_set_dirs: bool,
    should_set_backend: bool,
) -> Result<Vec<(&'static str, Option<String>)>, String> {
    let mut overrides: Vec<(&'static str, Option<String>)> =
        Vec::with_capacity(usize::from(should_set_dirs) * 2 + usize::from(should_set_backend));

    if should_set_dirs {
        let (runtime_dir, data_dir) =
            create_shared_pg_embed_dirs().map_err(|err| err.to_string())?;

        let runtime_dir_value = runtime_dir.to_string_lossy().into_owned();
        let data_dir_value = data_dir.to_string_lossy().into_owned();
        overrides.push(("PG_RUNTIME_DIR", Some(runtime_dir_value)));
        overrides.push(("PG_DATA_DIR", Some(data_dir_value)));
    }

    if should_set_backend {
        overrides.push(("PG_TEST_BACKEND", Some("postgresql_embedded".to_owned())));
    }

    Ok(overrides)
}

/// Bootstraps a shared [`ClusterHandle`] for persistent test sessions.
///
/// When `PG_RUNTIME_DIR`/`PG_DATA_DIR` are already set and usable, this
/// function leaves them untouched. If either value is missing or unusable,
/// this function sets both to stable directories under the target directory
/// so the shared cluster can be reused across multiple tests.
///
/// # Example
///
/// ```rust,ignore
/// let first = shared_cluster()?;
/// let second = shared_cluster()?;
/// assert!(std::ptr::eq(first, second));
/// # Ok::<(), String>(())
/// ```
pub fn shared_cluster() -> Result<&'static ClusterHandle, String> {
    let _bootstrap_guard = PG_EMBED_BOOTSTRAP_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|err| err.into_inner());

    if let Some(handle) = SHARED_CLUSTER_HANDLE.get() {
        return Ok(handle);
    }

    let should_override_dirs = !are_env_dirs_ready();
    let should_override_backend = std::env::var_os("PG_TEST_BACKEND").is_none();

    let _env_guard = if should_override_dirs || should_override_backend {
        let overrides = build_env_overrides(should_override_dirs, should_override_backend)?;

        Some(lock_env(overrides))
    } else {
        None
    };

    let cluster = bootstrap_with_retries(init_shared_cluster_handle)?;

    Ok(cluster)
}

/// Return whether the directory is usable, swallowing I/O errors.
fn ensure_dir(path: &Path) -> bool {
    if path.as_os_str().is_empty() {
        return false;
    }
    Dir::create_ambient_dir_all(path, ambient_authority()).is_ok()
}

#[cfg(test)]
mod tests {
    //! Regression coverage for pg-embed filesystem helpers.

    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_test_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time must be after UNIX_EPOCH")
            .as_nanos();
        pg_embed_target_dir().join(format!("{prefix}-{nanos}"))
    }

    #[test]
    fn is_transient_error_matches_known_patterns() {
        assert!(is_transient_error("error decoding response body"));
        assert!(is_transient_error("connection reset by peer"));
        assert!(is_transient_error("connection refused"));
        assert!(is_transient_error("request timeout"));
        assert!(is_transient_error("operation timed out"));
        assert!(is_transient_error("service temporarily unavailable"));
        assert!(is_transient_error("network unreachable"));
        assert!(is_transient_error("dns error: lookup failed"));
        assert!(is_transient_error("failed to lookup address"));
        assert!(is_transient_error("failed to connect to admin database"));
    }

    #[test]
    fn is_transient_error_is_case_insensitive() {
        assert!(is_transient_error("TIMEOUT"));
        assert!(is_transient_error("Connection Reset"));
        assert!(is_transient_error("DNS ERROR"));
        assert!(is_transient_error("TIMED OUT"));
        assert!(is_transient_error("Temporarily Unavailable"));
    }

    #[test]
    fn is_transient_error_rejects_non_transient_errors() {
        assert!(!is_transient_error("unknown error"));
        assert!(!is_transient_error("permission denied"));
        assert!(!is_transient_error("file not found"));
        assert!(!is_transient_error("invalid configuration"));
        assert!(!is_transient_error("authentication failed"));
        assert!(!is_transient_error(""));
    }

    #[test]
    fn ensure_dir_returns_false_for_empty_path() {
        assert!(!ensure_dir(Path::new("")));
    }

    #[test]
    fn ensure_dir_creates_directory_when_missing() {
        let dir = unique_test_dir("ensure-dir-create");
        remove_dir_all(&dir);
        assert!(ensure_dir(&dir));
        assert!(dir.exists());
        remove_dir_all(&dir);
    }

    #[test]
    fn ensure_dir_returns_true_when_directory_exists() {
        let dir = unique_test_dir("ensure-dir-existing");
        remove_dir_all(&dir);
        Dir::create_ambient_dir_all(&dir, ambient_authority())
            .expect("test directory should be creatable");
        assert!(ensure_dir(&dir));
        remove_dir_all(&dir);
    }

    fn remove_dir_all(path: &Path) {
        let Some(parent) = path.parent() else {
            return;
        };
        let Some(name) = path.file_name() else {
            return;
        };
        let Ok(dir) = Dir::open_ambient_dir(parent, ambient_authority()) else {
            return;
        };
        if dir.remove_dir_all(name).is_err() {
            // Ignore cleanup failures in test teardown.
        }
    }
}
