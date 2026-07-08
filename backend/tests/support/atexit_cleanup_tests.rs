//! Unit tests for atexit cleanup helpers.

use super::*;
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
    assert_eq!(
        super::unix_atexit::read_postmaster_pid(dir.path()),
        Some(12345)
    );
}

#[cfg(unix)]
#[test]
fn read_postmaster_pid_returns_none_for_missing_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert_eq!(super::unix_atexit::read_postmaster_pid(dir.path()), None);
}

#[cfg(unix)]
#[test]
fn read_postmaster_pid_returns_none_for_non_numeric_content() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_postmaster_pid(dir.path(), "not-a-number\n");
    assert_eq!(super::unix_atexit::read_postmaster_pid(dir.path()), None);
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
fn ensure_stable_cluster_environment_does_not_overwrite_existing_values() {
    let _guard = lock_pg_env(
        Some("custom_value"),
        Some("https://example.invalid/postgresql-binaries"),
    );
    super::ensure_stable_cluster_environment();
    assert_eq!(
        std::env::var("PG_PASSWORD").expect("PG_PASSWORD should be set"),
        "custom_value",
        "ensure_stable_cluster_environment should not overwrite an existing PG_PASSWORD"
    );
    assert_eq!(
        std::env::var("POSTGRESQL_RELEASES_URL").expect("POSTGRESQL_RELEASES_URL should be set"),
        "https://example.invalid/postgresql-binaries",
        "ensure_stable_cluster_environment should not overwrite an existing release URL"
    );
}

#[test]
fn ensure_stable_cluster_environment_sets_release_url_when_missing() {
    let _guard = lock_pg_env(Some("custom_value"), None);
    super::ensure_stable_cluster_environment();
    assert_eq!(
        std::env::var("POSTGRESQL_RELEASES_URL").expect("POSTGRESQL_RELEASES_URL should be set"),
        "https://github.com/theseus-rs/postgresql-binaries"
    );
}

#[test]
fn ensure_stable_cluster_environment_sets_password_when_missing() {
    let _guard = lock_pg_env(None, Some("https://example.invalid/postgresql-binaries"));
    super::ensure_stable_cluster_environment();
    assert_eq!(
        std::env::var("PG_PASSWORD").expect("PG_PASSWORD should be set"),
        "wildside_embedded_test",
        "ensure_stable_cluster_environment should set the stable default PG_PASSWORD"
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
