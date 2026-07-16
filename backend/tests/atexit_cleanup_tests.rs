//! Unit tests for the stable-cluster-environment helpers.
//!
//! These tests live in a dedicated integration-test target so they compile and
//! run exactly once. They include `support/stable_cluster_env.rs` directly —
//! rather than pulling the whole shared support surface in via
//! `declare_test_support!` — so this binary compiles only the helpers it
//! exercises. That keeps the target honest about its dependencies and avoids
//! any `#[allow(dead_code)]`/`#[allow(unused_imports)]` suppression: the
//! `libc::atexit` registration and cluster-handle acquisition it never calls
//! stay in `support/atexit_cleanup.rs` and are not compiled here.

#[path = "support/stable_cluster_env.rs"]
mod stable_cluster_env;

use std::time::Duration;

use cap_std::ambient_authority;
use cap_std::fs::Dir;
use rstest::rstest;
use stable_cluster_env::{SHARED_CLUSTER_RETRIES, SHARED_CLUSTER_RETRY_DELAY};

#[cfg(unix)]
fn write_postmaster_pid(dir_path: &std::path::Path, content: &str) {
    let dir = Dir::open_ambient_dir(dir_path, ambient_authority()).expect("open dir");
    dir.write("postmaster.pid", content).expect("write");
}

#[cfg(unix)]
#[rstest]
#[case::parses_first_line(Some("12345\n/some/path\n5432\n"), Some(12345))]
#[case::missing_file(None, None)]
#[case::non_numeric_content(Some("not-a-number\n"), None)]
fn read_postmaster_pid_reads_first_line(
    #[case] content: Option<&str>,
    #[case] expected: Option<i32>,
) {
    let dir = tempfile::tempdir().expect("tempdir");
    if let Some(content) = content {
        write_postmaster_pid(dir.path(), content);
    }
    assert_eq!(
        stable_cluster_env::unix_atexit::read_postmaster_pid(dir.path()),
        expected
    );
}

fn lock_pg_env(
    pg_password: Option<&'static str>,
    postgresql_releases_url: Option<&'static str>,
) -> env_lock::EnvGuard<'static> {
    env_lock::lock_env([
        ("PG_PASSWORD", pg_password),
        ("POSTGRESQL_RELEASES_URL", postgresql_releases_url),
    ])
}

#[test]
fn resolve_stable_env_does_not_overwrite_existing_values() {
    let _guard = lock_pg_env(
        Some("custom_value"),
        Some("https://example.invalid/postgresql-binaries"),
    );
    stable_cluster_env::resolve_stable_env();
    assert_eq!(
        std::env::var("PG_PASSWORD").expect("PG_PASSWORD should be set"),
        "custom_value",
        "resolve_stable_env should not overwrite an existing PG_PASSWORD"
    );
    assert_eq!(
        std::env::var("POSTGRESQL_RELEASES_URL").expect("POSTGRESQL_RELEASES_URL should be set"),
        "https://example.invalid/postgresql-binaries",
        "resolve_stable_env should not overwrite an existing release URL"
    );
}

#[test]
fn resolve_stable_env_sets_release_url_when_missing() {
    let _guard = lock_pg_env(Some("custom_value"), None);
    stable_cluster_env::resolve_stable_env();
    assert_eq!(
        std::env::var("POSTGRESQL_RELEASES_URL").expect("POSTGRESQL_RELEASES_URL should be set"),
        "https://github.com/theseus-rs/postgresql-binaries"
    );
}

#[test]
fn resolve_stable_env_sets_password_when_missing() {
    let _guard = lock_pg_env(None, Some("https://example.invalid/postgresql-binaries"));
    stable_cluster_env::resolve_stable_env();
    assert_eq!(
        std::env::var("PG_PASSWORD").expect("PG_PASSWORD should be set"),
        "wildside_embedded_test",
        "resolve_stable_env should set the stable default PG_PASSWORD"
    );
}

#[cfg(unix)]
#[test]
fn ensure_stable_cluster_environment_resolves_env_once_under_concurrency() {
    // Concurrent callers of `ensure_stable_cluster_environment` must observe a
    // consistent stable environment without racing or panicking. First-call
    // initialization (which runs `std::env::set_var`) happens once on this
    // single thread *before* any threads are spawned, so `set_var` never
    // executes while other threads exist. The spawned threads then exercise only
    // the cached, post-initialization path guarded by the `STABLE_ENV_INIT`
    // OnceLock.
    let sandbox = tempfile::tempdir().expect("tempdir");
    let install_path = sandbox.path().join("install");
    let data_parent_path = sandbox.path().join("data-parent");
    Dir::create_ambient_dir_all(&install_path, ambient_authority()).expect("create install dir");
    Dir::create_ambient_dir_all(&data_parent_path, ambient_authority())
        .expect("create data parent");

    // Clear the resolved variables so the OnceLock closure applies the defaults,
    // and sandbox the repair paths so the incidental repair stays hermetic.
    let _guard = env_lock::lock_env([
        ("PG_PASSWORD", None),
        ("POSTGRESQL_RELEASES_URL", None),
        (
            "PG_RUNTIME_DIR",
            Some(
                install_path
                    .to_str()
                    .expect("install path is valid UTF-8")
                    .to_owned(),
            ),
        ),
        (
            "PG_DATA_DIR",
            Some(
                data_parent_path
                    .join("data")
                    .to_str()
                    .expect("data path is valid UTF-8")
                    .to_owned(),
            ),
        ),
    ]);

    // Perform first-call initialization on the current (single) thread. The
    // process-global `STABLE_ENV_INIT` OnceLock resolves and `set_var` runs
    // while single-threaded, which is sound.
    stable_cluster_env::ensure_stable_cluster_environment()
        .expect("reconcile stable cluster environment before cluster access");

    // The spawned threads now hit only the cached post-initialization path (no
    // `set_var`), proving concurrent callers observe consistent state without
    // racing or panicking.
    std::thread::scope(|scope| {
        for _ in 0..8 {
            scope.spawn(stable_cluster_env::ensure_stable_cluster_environment);
        }
    });

    // `STABLE_ENV_INIT` is process-global and every caller resolves to the same
    // stable default, so these assertions hold regardless of which test
    // initialized it first (order-independent).
    assert_eq!(
        std::env::var("PG_PASSWORD").expect("PG_PASSWORD should be set"),
        "wildside_embedded_test",
        "concurrent resolution must apply the stable default PG_PASSWORD exactly once",
    );
    assert_eq!(
        std::env::var("POSTGRESQL_RELEASES_URL").expect("POSTGRESQL_RELEASES_URL should be set"),
        "https://github.com/theseus-rs/postgresql-binaries",
        "concurrent resolution must pin the release URL exactly once",
    );
}

#[cfg(unix)]
#[test]
fn ensure_stable_cluster_environment_serializes_concurrent_repair() {
    // Two threads repairing the same stale password file concurrently must not
    // race on removing it. Without the process-local repair lock, the second
    // `remove_file` observes the file already gone and panics via `.expect`.
    let sandbox = tempfile::tempdir().expect("tempdir");
    let install_path = sandbox.path().join("install");
    let data_parent_path = sandbox.path().join("data-parent");
    let data_dir_path = data_parent_path.join("data");
    Dir::create_ambient_dir_all(&install_path, ambient_authority()).expect("create install dir");
    Dir::create_ambient_dir_all(&data_parent_path, ambient_authority())
        .expect("create data parent");

    let install_dir =
        Dir::open_ambient_dir(&install_path, ambient_authority()).expect("open install");
    install_dir
        .write(".pgpass", b"stale-password")
        .expect("seed stale password file");

    // Pre-set every variable the repair path reads so the worker threads only
    // read the environment (never `set_var`) and target the sandbox. A custom
    // `PG_DATA_DIR` keeps `should_remove_data_dir` false, isolating the race to
    // the `.pgpass` removal.
    let _guard = env_lock::lock_env([
        ("PG_PASSWORD", Some("wildside_embedded_test".to_owned())),
        (
            "POSTGRESQL_RELEASES_URL",
            Some("https://example.invalid/postgresql-binaries".to_owned()),
        ),
        (
            "PG_RUNTIME_DIR",
            Some(
                install_path
                    .to_str()
                    .expect("install path is valid UTF-8")
                    .to_owned(),
            ),
        ),
        (
            "PG_DATA_DIR",
            Some(
                data_dir_path
                    .to_str()
                    .expect("data path is valid UTF-8")
                    .to_owned(),
            ),
        ),
    ]);

    std::thread::scope(|scope| {
        for _ in 0..2 {
            scope.spawn(stable_cluster_env::ensure_stable_cluster_environment);
        }
    });

    assert!(
        !install_dir.exists(".pgpass"),
        "concurrent repair must remove the stale password file exactly once \
         without panicking",
    );
}

#[test]
fn retry_budget_is_within_expected_bounds() {
    let retry_count = std::hint::black_box(SHARED_CLUSTER_RETRIES);
    let retry_delay = std::hint::black_box(SHARED_CLUSTER_RETRY_DELAY);

    assert_eq!(
        retry_count, 5,
        "SHARED_CLUSTER_RETRIES must equal 5; got {retry_count}"
    );
    assert_eq!(
        retry_delay,
        Duration::from_millis(500),
        "SHARED_CLUSTER_RETRY_DELAY must equal 500 ms; got {retry_delay:?}"
    );
}
