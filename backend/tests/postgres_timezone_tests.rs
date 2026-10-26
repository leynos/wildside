//! Unit tests for embedded PostgreSQL timezone compatibility.

#[path = "support/postgres_timezone.rs"]
mod postgres_timezone;

use std::io;

use cap_std::fs::Dir;

#[test]
fn creates_utc_alias_from_the_bundled_uct_timezone() -> io::Result<()> {
    let sandbox = tempfile::tempdir()?;
    let timezone_dir = Dir::open_ambient_dir(sandbox.path(), cap_std::ambient_authority())?;
    timezone_dir.write("UCT", b"zero-offset timezone data")?;

    postgres_timezone::ensure_utc_alias(&timezone_dir)?;

    assert_eq!(timezone_dir.read("UTC")?, b"zero-offset timezone data");
    Ok(())
}

#[test]
fn preserves_an_existing_utc_timezone_file() -> io::Result<()> {
    let sandbox = tempfile::tempdir()?;
    let timezone_dir = Dir::open_ambient_dir(sandbox.path(), cap_std::ambient_authority())?;
    timezone_dir.write("UCT", b"UCT data")?;
    timezone_dir.write("UTC", b"existing UTC data")?;

    postgres_timezone::ensure_utc_alias(&timezone_dir)?;

    assert_eq!(timezone_dir.read("UTC")?, b"existing UTC data");
    Ok(())
}

#[test]
fn reports_when_neither_timezone_name_is_available() -> io::Result<()> {
    let sandbox = tempfile::tempdir()?;
    let timezone_dir = Dir::open_ambient_dir(sandbox.path(), cap_std::ambient_authority())?;

    let error = postgres_timezone::ensure_utc_alias(&timezone_dir)
        .expect_err("missing UTC and UCT data must stop test setup");

    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    Ok(())
}
