//! Password-file repair for embedded PostgreSQL integration tests.
//!
//! `postgresql_embedded` only writes the initdb password file when it does not
//! already exist. When the shared stable test password changes, a stale
//! `.pgpass` can initialize a fresh data directory with the old password while
//! clients connect with the new one.

use std::io;
use std::path::{Path, PathBuf};

use cap_std::ambient_authority;
use cap_std::fs::Dir;

pub(super) fn repair_default_password_state(password: &[u8]) -> io::Result<()> {
    let paths = PasswordStatePaths::from_environment();
    // A missing install/data directory means there is nothing to repair, so
    // treat it as a quiet success. Genuine failures (permissions, a path that
    // is not a directory, and so on) are propagated so the caller can surface
    // them at an explicit boundary rather than mistaking them for absent state.
    let Some(install_dir) = open_dir_if_exists(&paths.install_dir)? else {
        return Ok(());
    };
    let Some(data_parent) = open_dir_if_exists(&paths.data_parent)? else {
        return Ok(());
    };

    repair_password_file_state(password, &install_dir, &data_parent, &paths)
}

/// Open an ambient directory, treating only a missing path as a quiet no-op.
///
/// Returns `Ok(None)` when the directory does not exist, `Ok(Some(dir))` when
/// it opens successfully, and propagates every other I/O error (for example a
/// permission failure or a non-directory path) instead of hiding it as absent.
fn open_dir_if_exists(path: &Path) -> io::Result<Option<Dir>> {
    match Dir::open_ambient_dir(path, ambient_authority()) {
        Ok(dir) => Ok(Some(dir)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

struct PasswordStatePaths {
    install_dir: PathBuf,
    data_parent: PathBuf,
    data_name: PathBuf,
    should_remove_data_dir: bool,
}

impl PasswordStatePaths {
    fn from_environment() -> Self {
        // SAFETY: `geteuid` has no preconditions and does not modify memory.
        let uid = unsafe { libc::geteuid() };
        let base = PathBuf::from(format!("/var/tmp/pg-embed-{uid}"));
        let default_install_dir = base.join("install");
        let default_data_dir = base.join("data");
        let data_dir = std::env::var_os("PG_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or(default_data_dir);
        let install_dir = std::env::var_os("PG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or(default_install_dir);
        let should_remove_data_dir = std::env::var_os("PG_DATA_DIR").is_none();

        let data_parent = data_dir
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_owned();
        let data_name = data_dir
            .file_name()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("data"));

        Self {
            install_dir,
            data_parent,
            data_name,
            should_remove_data_dir,
        }
    }
}

fn repair_password_file_state(
    password: &[u8],
    install_dir: &Dir,
    data_parent: &Dir,
    paths: &PasswordStatePaths,
) -> io::Result<()> {
    let existing_password = match install_dir.read(Path::new(".pgpass")) {
        Ok(contents) => contents,
        // No stale password file means nothing to repair.
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        // Permission or other read failures must not be mistaken for absence.
        Err(error) => return Err(error),
    };
    if existing_password == password {
        return Ok(());
    }

    install_dir.remove_file(Path::new(".pgpass"))?;

    if paths.should_remove_data_dir {
        remove_dir_if_exists(data_parent, &paths.data_name)?;
    }

    Ok(())
}

fn remove_dir_if_exists(parent: &Dir, path: &Path) -> io::Result<()> {
    match parent.remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
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
        fn new(should_remove_data_dir: bool) -> Self {
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
            data_parent.create_dir("data").expect("create data dir");

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

        repair_password_file_state(
            repaired_password,
            &fixture.install_dir,
            &fixture.data_parent,
            &fixture.paths,
        )
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
        let not_a_dir = sandbox.path().join("not-a-dir");
        std::fs::write(&not_a_dir, b"x").expect("write file");
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
        let error = repair_password_file_state(
            b"new-password",
            &fixture.install_dir,
            &fixture.data_parent,
            &fixture.paths,
        )
        .expect_err("reading a directory as a file must fail");
        assert_ne!(
            error.kind(),
            io::ErrorKind::NotFound,
            "an unreadable password file must surface as a real error"
        );
    }
}
