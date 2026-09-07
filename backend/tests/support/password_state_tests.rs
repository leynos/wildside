//! Unit tests for stale embedded PostgreSQL password-state repair and for
//! repair-path resolution through an injected reader.

use super::*;
use rstest::rstest;
use std::collections::HashMap;

struct PasswordStateFixture {
    _sandbox: tempfile::TempDir,
    install_dir: Dir,
    data_parent: Dir,
    paths: PasswordStatePaths,
}

impl PasswordStateFixture {
    /// Build a fixture whose data parent is seeded by `create_data_entry`.
    fn with_data_entry(should_remove_data_dir: bool, create_data_entry: impl FnOnce(&Dir)) -> Self {
        let sandbox = tempfile::tempdir().expect("tempdir");
        let install_path = sandbox.path().join("install");
        let data_parent_path = sandbox.path().join("data-parent");
        Dir::create_ambient_dir_all(&install_path, ambient_authority())
            .expect("create install dir");
        Dir::create_ambient_dir_all(&data_parent_path, ambient_authority())
            .expect("create data parent");
        let install_dir =
            Dir::open_ambient_dir(&install_path, ambient_authority()).expect("open install");
        let data_parent = Dir::open_ambient_dir(&data_parent_path, ambient_authority())
            .expect("open data parent");

        create_data_entry(&data_parent);

        Self {
            _sandbox: sandbox,
            install_dir,
            data_parent,
            paths: PasswordStatePaths {
                install_dir: install_path,
                data_parent: data_parent_path,
                data_name: PathBuf::from("data"),
                should_remove_data_dir,
            },
        }
    }

    /// Build a fixture whose data parent holds an ordinary `data` directory.
    fn new(should_remove_data_dir: bool) -> Self {
        Self::with_data_entry(should_remove_data_dir, |data_parent| {
            data_parent.create_dir("data").expect("create data dir");
        })
    }

    /// Build a fixture whose data parent directory does not exist on disk.
    ///
    /// The install directory (and its `.pgpass`) is still created so stale
    /// password cleanup can be exercised, but `paths.data_parent` points at
    /// an absent path to prove the repair does not depend on it.
    fn with_absent_data_parent() -> Self {
        let sandbox = tempfile::tempdir().expect("tempdir");
        let install_path = sandbox.path().join("install");
        let data_parent_path = sandbox.path().join("absent-data-parent");
        Dir::create_ambient_dir_all(&install_path, ambient_authority())
            .expect("create install dir");
        let install_dir =
            Dir::open_ambient_dir(&install_path, ambient_authority()).expect("open install");
        // Open the sandbox root as a stand-in handle for `data_parent`; the
        // absent path in `paths` is what the repair actually resolves.
        let data_parent =
            Dir::open_ambient_dir(sandbox.path(), ambient_authority()).expect("open sandbox");

        Self {
            _sandbox: sandbox,
            install_dir,
            data_parent,
            paths: PasswordStatePaths {
                install_dir: install_path,
                data_parent: data_parent_path,
                data_name: PathBuf::from("data"),
                should_remove_data_dir: true,
            },
        }
    }

    /// Build a fixture whose data entry is a regular file, not a directory.
    ///
    /// Removing it via `remove_dir_all` then fails with a non-NotFound error
    /// (ENOTDIR), exercising the path where data cleanup fails while the
    /// stale `.pgpass` must remain in place for a retry.
    fn with_data_entry_as_file() -> Self {
        Self::with_data_entry(true, |data_parent| {
            // The "data" entry is a file, so `remove_dir_all` fails with a
            // non-NotFound error rather than deleting a directory.
            data_parent.write("data", b"x").expect("write data file");
        })
    }

    /// Seed the install directory's `.pgpass` with `contents`.
    fn write_pgpass(&self, contents: &[u8]) {
        self.install_dir
            .write(".pgpass", contents)
            .expect("write pgpass");
    }
}

struct ExpectedPasswordState {
    should_keep_pgpass: bool,
    should_keep_data: bool,
}

#[rstest]
#[case::stale_default(
    PasswordStateFixture::new(true),
    b"old-password".as_slice(),
    b"new-password".as_slice(),
    ExpectedPasswordState {
        should_keep_pgpass: false,
        should_keep_data: false,
    },
)]
#[case::matching_default(
    PasswordStateFixture::new(true),
    b"same-password".as_slice(),
    b"same-password".as_slice(),
    ExpectedPasswordState {
        should_keep_pgpass: true,
        should_keep_data: true,
    },
)]
#[case::custom_data(
    PasswordStateFixture::new(false),
    b"old-password".as_slice(),
    b"new-password".as_slice(),
    ExpectedPasswordState {
        should_keep_pgpass: false,
        should_keep_data: true,
    },
)]
fn repair_password_file_state_handles_password_state(
    #[case] fixture: PasswordStateFixture,
    #[case] written_password: &[u8],
    #[case] repaired_password: &[u8],
    #[case] expected: ExpectedPasswordState,
) {
    fixture.write_pgpass(written_password);

    repair_password_file_state(repaired_password, &fixture.install_dir, &fixture.paths)
        .expect("repair should succeed");

    assert_eq!(
        fixture.install_dir.exists(".pgpass"),
        expected.should_keep_pgpass,
        "password file existence should match the scenario expectation"
    );
    assert_eq!(
        fixture.data_parent.exists("data"),
        expected.should_keep_data,
        "data dir existence should match the scenario expectation"
    );
}

#[test]
fn repair_removes_stale_pgpass_when_data_parent_is_absent() {
    // A stale `.pgpass` must be repaired even when the default data parent
    // directory does not exist. The absent parent only governs data-dir
    // deletion, so it must not short-circuit the `.pgpass` cleanup.
    let fixture = PasswordStateFixture::with_absent_data_parent();
    fixture.write_pgpass(b"old-password");
    assert!(
        !fixture.paths.data_parent.exists(),
        "the data parent must be absent for this scenario to be meaningful"
    );

    repair_password_file_state(b"new-password", &fixture.install_dir, &fixture.paths)
        .expect("repair should succeed even without a data parent");

    assert!(
        !fixture.install_dir.exists(".pgpass"),
        "the stale password file should be removed despite the absent data parent"
    );
}

#[test]
fn repair_keeps_pgpass_when_data_cleanup_fails() {
    // When the data-directory removal fails, `.pgpass` must remain so a
    // retry still detects the stale state. Cleaning the data directory
    // before deleting `.pgpass` keeps the repair retryable.
    let fixture = PasswordStateFixture::with_data_entry_as_file();
    fixture.write_pgpass(b"old-password");

    let error = repair_password_file_state(b"new-password", &fixture.install_dir, &fixture.paths)
        .expect_err("data cleanup failure must propagate");
    assert_ne!(
        error.kind(),
        io::ErrorKind::NotFound,
        "a failed data removal must surface as a real error"
    );
    assert!(
        fixture.install_dir.exists(".pgpass"),
        "the stale password file must remain so the repair stays retryable"
    );
}

#[test]
fn open_dir_if_exists_reports_absent_directory_as_none() {
    let sandbox = tempfile::tempdir().expect("tempdir");
    let missing = sandbox.path().join("missing");
    assert!(
        open_dir_if_exists(&missing)
            .expect("a missing directory must be a quiet no-op")
            .is_none(),
        "an absent directory should resolve to None rather than erroring"
    );
}

#[test]
fn open_dir_if_exists_propagates_non_not_found_errors() {
    let sandbox = tempfile::tempdir().expect("tempdir");
    // A regular file is not a directory, so opening it fails with a
    // non-NotFound error that must be propagated rather than swallowed.
    // Create the file through a cap-std handle to honour the ambient-
    // authority filesystem policy rather than touching std::fs directly.
    let sandbox_dir =
        Dir::open_ambient_dir(sandbox.path(), ambient_authority()).expect("open sandbox");
    sandbox_dir.write("not-a-dir", b"x").expect("write file");
    let not_a_dir = sandbox.path().join("not-a-dir");
    let error =
        open_dir_if_exists(&not_a_dir).expect_err("opening a file as a directory must fail");
    assert_ne!(
        error.kind(),
        io::ErrorKind::NotFound,
        "a non-directory path must surface as a real error, not absent state"
    );
}

#[test]
fn repair_password_file_state_propagates_non_not_found_read_errors() {
    let fixture = PasswordStateFixture::new(true);
    // Make `.pgpass` a directory so reading it as a file fails with a
    // non-NotFound error that must propagate rather than look absent.
    fixture
        .install_dir
        .create_dir(".pgpass")
        .expect("create .pgpass directory");
    let error = repair_password_file_state(b"new-password", &fixture.install_dir, &fixture.paths)
        .expect_err("reading a directory as a file must fail");
    assert_ne!(
        error.kind(),
        io::ErrorKind::NotFound,
        "an unreadable password file must surface as a real error"
    );
}

/// Build a stub environment reader over a fixed variable list.
fn stub_env(vars: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
    let vars: HashMap<&str, &str> = vars.iter().copied().collect();
    move |name| vars.get(name).map(|value| (*value).to_owned())
}

/// Scenario: `PG_DATA_DIR` and `PG_RUNTIME_DIR` are absent or supplied by
/// the injected reader.
///
/// Invariant: absent variables select the per-uid defaults under
/// `/var/tmp`, an explicit data directory suppresses deletion, and the
/// resolution never touches the process environment.
#[rstest]
#[case::defaults(&[], true)]
#[case::explicit_data_dir(&[("PG_DATA_DIR", "/tmp/wildside-data/data")], false)]
#[case::explicit_runtime_dir(&[("PG_RUNTIME_DIR", "/tmp/wildside-install")], true)]
fn resolve_reads_overrides_from_the_injected_reader(
    #[case] vars: &'static [(&'static str, &'static str)],
    #[case] expected_should_remove_data_dir: bool,
) {
    let paths = PasswordStatePaths::resolve(stub_env(vars));

    assert_eq!(
        paths.should_remove_data_dir, expected_should_remove_data_dir,
        "only a default data directory may be deleted by the repair"
    );
    let expected_install: Option<PathBuf> = vars
        .iter()
        .find(|(name, _)| *name == "PG_RUNTIME_DIR")
        .map(|(_, value)| PathBuf::from(*value));
    if let Some(expected_install) = expected_install {
        assert_eq!(
            paths.install_dir, expected_install,
            "an explicit PG_RUNTIME_DIR must be used verbatim"
        );
    } else {
        assert!(
            paths.install_dir.ends_with("install"),
            "the default install directory must sit under the per-uid base"
        );
    }
}
