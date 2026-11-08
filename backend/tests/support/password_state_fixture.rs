//! Shared fixture for the password-state repair tests.
//!
//! The fixture owns a sandbox with an install directory and a data parent, so
//! each test can seed only the state its scenario is about.

use super::*;

pub(super) struct PasswordStateFixture {
    _sandbox: tempfile::TempDir,
    pub(super) install_dir: Dir,
    pub(super) data_parent: Dir,
    pub(super) paths: PasswordStatePaths,
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
    pub(super) fn new(should_remove_data_dir: bool) -> Self {
        Self::with_data_entry(should_remove_data_dir, |data_parent| {
            data_parent.create_dir("data").expect("create data dir");
        })
    }

    /// Build a fixture whose data parent directory does not exist on disk.
    ///
    /// The install directory (and its `.pgpass`) is still created so stale
    /// password cleanup can be exercised, but `paths.data_parent` points at
    /// an absent path to prove the repair does not depend on it.
    pub(super) fn with_absent_data_parent() -> Self {
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
    pub(super) fn with_data_entry_as_file() -> Self {
        Self::with_data_entry(true, |data_parent| {
            // The "data" entry is a file, so `remove_dir_all` fails with a
            // non-NotFound error rather than deleting a directory.
            data_parent.write("data", b"x").expect("write data file");
        })
    }

    /// Seed the install directory's `.pgpass` with `contents`.
    pub(super) fn write_pgpass(&self, contents: &[u8]) {
        self.install_dir
            .write(".pgpass", contents)
            .expect("write pgpass");
    }

    /// Declare the major version the retained data directory must match.
    pub(super) fn requiring_major_version(mut self, major: Option<u32>) -> Self {
        self.paths.expected_major_version = major;
        self
    }

    /// Seed the data directory's `PG_VERSION` with `contents`.
    pub(super) fn write_pg_version(&self, contents: &str) {
        self.write_data_file("PG_VERSION", contents);
    }

    /// Mark the data directory as initialized.
    ///
    /// `postgresql_embedded` uses this file, and nothing else, to decide
    /// whether a bootstrap may reuse an existing data directory.
    pub(super) fn write_postgresql_conf(&self) {
        self.write_data_file("postgresql.conf", "# initialized\n");
    }

    /// Write `contents` to `name` inside the fixture's data directory.
    pub(super) fn write_data_file(&self, name: &str, contents: &str) {
        let data_dir = self
            .data_parent
            .open_dir("data")
            .expect("the fixture's data directory exists");
        data_dir.write(name, contents).expect("write data file");
    }
}
