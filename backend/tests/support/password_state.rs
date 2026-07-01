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

pub(super) fn repair_default_password_state(password: &[u8]) {
    let paths = PasswordStatePaths::from_environment();
    let Ok(install_dir) = Dir::open_ambient_dir(&paths.install_dir, ambient_authority()) else {
        return;
    };
    let Ok(data_parent) = Dir::open_ambient_dir(&paths.data_parent, ambient_authority()) else {
        return;
    };

    repair_password_file_state(password, &install_dir, &data_parent, &paths);
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
) {
    let Ok(existing_password) = install_dir.read(Path::new(".pgpass")) else {
        return;
    };
    if existing_password == password {
        return;
    }

    install_dir
        .remove_file(Path::new(".pgpass"))
        .expect("remove stale embedded PostgreSQL password file");

    if paths.should_remove_data_dir {
        remove_dir_if_exists(data_parent, &paths.data_name)
            .expect("remove stale embedded PostgreSQL data directory");
    }
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
        );

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
}
