//! Unit tests for the stable-cluster password helpers.
//!
//! These tests live in a dedicated integration-test target so they compile and
//! run exactly once. They include `support/stable_cluster_env.rs` directly —
//! rather than pulling the whole shared support surface in via
//! `declare_test_support!` — so this binary compiles only the helpers it
//! exercises. That keeps the target honest about its dependencies and avoids
//! any `#[allow(dead_code)]`/`#[allow(unused_imports)]` suppression: the
//! `libc::atexit` registration and cluster-handle acquisition it never calls
//! stay in `support/atexit_cleanup.rs` and are not compiled here.
//!
//! Every test drives the helpers through an injected reader and explicit
//! sandbox paths. Nothing here reads or writes the process environment, so the
//! target needs no serialization and no runtime-ordering contract.

#[path = "support/stable_cluster_env.rs"]
mod stable_cluster_env;

use std::collections::HashMap;
#[cfg(unix)]
use std::path::PathBuf;
use std::time::Duration;

#[cfg(unix)]
use cap_std::ambient_authority;
#[cfg(unix)]
use cap_std::fs::Dir;
use rstest::rstest;
use stable_cluster_env::{DEFAULT_PG_PASSWORD, SHARED_CLUSTER_RETRIES, SHARED_CLUSTER_RETRY_DELAY};

#[cfg(unix)]
fn write_postmaster_pid(dir_path: &std::path::Path, content: &str) {
    let dir = Dir::open_ambient_dir(dir_path, ambient_authority()).expect("open dir");
    dir.write("postmaster.pid", content).expect("write");
}

/// Build a stub environment reader over a fixed variable map.
fn stub_env(vars: HashMap<&'static str, String>) -> impl Fn(&str) -> Option<String> {
    move |name| vars.get(name).cloned()
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

/// Scenario: `PG_PASSWORD` is supplied by the reader, absent, or empty.
///
/// Invariant: an existing value is returned unchanged and every other case
/// yields the stable default. The resolver only reads the injected map, so the
/// process environment is neither consulted nor modified.
#[rstest]
#[case::existing_value_is_preserved(Some("custom_value"), "custom_value")]
#[case::absent_uses_the_default(None, DEFAULT_PG_PASSWORD)]
#[case::empty_uses_the_default(Some(""), DEFAULT_PG_PASSWORD)]
fn resolve_stable_password_prefers_an_existing_value(
    #[case] configured: Option<&str>,
    #[case] expected: &str,
) {
    let vars = configured.map_or_else(HashMap::new, |value| {
        HashMap::from([("PG_PASSWORD", value.to_owned())])
    });

    assert_eq!(
        stable_cluster_env::resolve_stable_password(stub_env(vars)),
        expected,
        "the resolver must preserve a configured password and otherwise apply \
         the stable default"
    );
}

/// A sandboxed install/data directory pair standing in for the shared cluster's
/// on-disk state.
#[cfg(unix)]
struct ClusterStateSandbox {
    _sandbox: tempfile::TempDir,
    install_path: PathBuf,
    data_path: PathBuf,
    install_dir: Dir,
}

#[cfg(unix)]
impl ClusterStateSandbox {
    /// Create the sandbox with an empty install and data directory pair.
    fn new() -> Self {
        let sandbox = tempfile::tempdir().expect("tempdir");
        let install_path = sandbox.path().join("install");
        let data_path = sandbox.path().join("data-parent").join("data");
        Dir::create_ambient_dir_all(&install_path, ambient_authority())
            .expect("create install dir");
        Dir::create_ambient_dir_all(&data_path, ambient_authority()).expect("create data dir");
        let install_dir =
            Dir::open_ambient_dir(&install_path, ambient_authority()).expect("open install");

        Self {
            _sandbox: sandbox,
            install_path,
            data_path,
            install_dir,
        }
    }

    /// Repair paths pointing at this sandbox.
    ///
    /// An explicit `PG_DATA_DIR` marks the data directory as caller-owned, so
    /// only `.pgpass` removal is exercised.
    fn paths(&self) -> stable_cluster_env::PasswordStatePaths {
        stable_cluster_env::PasswordStatePaths::resolve(self.reader("unused-password"))
    }

    /// A reader that points the resolver at this sandbox.
    fn reader(&self, password: &str) -> impl Fn(&str) -> Option<String> + use<> {
        stub_env(HashMap::from([
            ("PG_PASSWORD", password.to_owned()),
            (
                "PG_RUNTIME_DIR",
                self.install_path
                    .to_str()
                    .expect("install path is valid UTF-8")
                    .to_owned(),
            ),
            (
                "PG_DATA_DIR",
                self.data_path
                    .to_str()
                    .expect("data path is valid UTF-8")
                    .to_owned(),
            ),
        ]))
    }
}

/// Scenario: the cluster's existing `.pgpass` was written under a different
/// password, or under the one the reader resolves.
///
/// Invariant: `ensure_stable_cluster_environment_with` carries the reader's
/// resolved password into the repair, so a mismatched file is removed and a
/// matching one survives. A warm cluster is therefore not torn down on every
/// setup call.
#[cfg(unix)]
#[rstest]
#[case::stale_password_is_cleared(b"stale-password", false)]
#[case::matching_password_is_kept(b"resolved-password", true)]
fn ensure_stable_cluster_environment_repairs_with_the_resolved_password(
    #[case] existing_password: &[u8],
    #[case] should_keep_password_file: bool,
) {
    let sandbox = ClusterStateSandbox::new();
    sandbox
        .install_dir
        .write(".pgpass", existing_password)
        .expect("seed the existing password file");

    stable_cluster_env::ensure_stable_cluster_environment_with(sandbox.reader("resolved-password"))
        .expect("repair password state");

    assert_eq!(
        sandbox.install_dir.exists(".pgpass"),
        should_keep_password_file,
        "the repair must compare the resolved password against the existing file"
    );
}

/// Scenario: two threads repair the same stale password file at once.
///
/// Invariant: the process-local repair lock serializes the `.pgpass` removal,
/// so neither thread observes an already-removed file and the file ends up
/// gone exactly once.
#[cfg(unix)]
#[test]
fn repair_password_state_serializes_concurrent_callers() {
    let sandbox = ClusterStateSandbox::new();
    let paths = sandbox.paths();
    sandbox
        .install_dir
        .write(".pgpass", b"stale-password")
        .expect("seed stale password file");

    std::thread::scope(|scope| {
        for _ in 0..8 {
            scope.spawn(|| {
                stable_cluster_env::repair_password_state_serialized(
                    b"wildside_embedded_test",
                    &paths,
                )
                .expect("repair stale password state");
            });
        }
    });

    assert!(
        !sandbox.install_dir.exists(".pgpass"),
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
