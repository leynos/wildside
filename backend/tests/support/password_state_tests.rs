//! Unit tests for stale embedded PostgreSQL password-state repair.

use super::*;
use rstest::rstest;

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
                expected_major_version: None,
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
                expected_major_version: None,
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

    /// Declare the major version the retained data directory must match.
    fn requiring_major_version(mut self, major: Option<u32>) -> Self {
        self.paths.expected_major_version = major;
        self
    }

    /// Seed the data directory's `PG_VERSION` with `contents`.
    fn write_pg_version(&self, contents: &str) {
        self.write_data_file("PG_VERSION", contents);
    }

    /// Mark the data directory as initialized.
    ///
    /// `postgresql_embedded` uses this file, and nothing else, to decide
    /// whether a bootstrap may reuse an existing data directory.
    fn write_postgresql_conf(&self) {
        self.write_data_file("postgresql.conf", "# initialized\n");
    }

    /// Write `contents` to `name` inside the fixture's data directory.
    fn write_data_file(&self, name: &str, contents: &str) {
        let data_dir = self
            .data_parent
            .open_dir("data")
            .expect("the fixture's data directory exists");
        data_dir.write(name, contents).expect("write data file");
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

/// Scenario: a retained default data directory the next bootstrap cannot reuse.
///
/// Invariant: only the default data directory is removed, and only when a
/// bootstrap could not reuse it — because it was never initialized, or because
/// its stored `PG_VERSION` disagrees with the configured major version.
#[rstest]
// A bootstrap that never finished writes `PG_VERSION` but no
// `postgresql.conf`, so the library re-runs `initdb` over a non-empty
// directory and fails.
#[case::aborted_initialization_removed(Some(16), Some("16"), false, true)]
#[case::never_started_removed(Some(16), None, false, true)]
#[case::older_version_removed(Some(16), Some("17"), true, true)]
#[case::matching_version_kept(Some(16), Some("16"), true, false)]
// Without a requirement the stored version cannot be contradicted, so an
// initialized directory is kept whatever it holds.
#[case::unpinned_initialized_directory_kept(None, Some("17"), true, false)]
fn repair_unusable_data_directory_handles_retained_state(
    #[case] expected: Option<u32>,
    #[case] stored: Option<&str>,
    #[case] is_initialized: bool,
    #[case] should_remove_data: bool,
) {
    let fixture = PasswordStateFixture::new(true).requiring_major_version(expected);
    if let Some(stored) = stored {
        fixture.write_pg_version(stored);
    }
    if is_initialized {
        fixture.write_postgresql_conf();
    }

    repair_unusable_data_directory(&fixture.paths).expect("data directory repair should succeed");

    assert_eq!(
        fixture.data_parent.exists("data"),
        !should_remove_data,
        "a retained directory must be removed only when a bootstrap cannot reuse it"
    );
}

#[test]
fn repair_unusable_data_directory_keeps_an_explicit_data_directory() {
    // The fixture's data directory has no `postgresql.conf`, so it is the
    // state the repair would otherwise delete; an explicit `PG_DATA_DIR`
    // belongs to the caller and must survive.
    let fixture = PasswordStateFixture::new(false).requiring_major_version(Some(16));

    repair_unusable_data_directory(&fixture.paths).expect("data directory repair should succeed");

    assert!(
        fixture.data_parent.exists("data"),
        "an explicitly configured data directory belongs to the caller"
    );
}

#[test]
fn major_version_reads_cargo_requirements_and_postgres_files_alike() {
    assert_eq!(major_version("=16.10.0"), Some(16));
    assert_eq!(major_version("16.10.0"), Some(16));
    assert_eq!(major_version(">=17"), Some(17));
    assert_eq!(major_version(" 18\n"), Some(18));
    assert_eq!(major_version(""), None);
    assert_eq!(major_version("latest"), None);
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
