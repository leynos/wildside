//! Tests for embedded PostgreSQL UTC readiness and bundled timezone repair.

#[path = "support/postgres_timezone.rs"]
mod postgres_timezone;

use std::cell::Cell;
use std::io;
use std::path::Path;

use cap_std::fs::Dir;
use color_eyre::eyre::Result;
use color_eyre::eyre::eyre;
use postgres::error::SqlState;
use postgres_timezone::ProbeOutcome;
use rstest::{fixture, rstest};

/// A temporary directory paired with an ambient-authority handle to it.
///
/// The guard travels with the handle so the directory outlives the test body.
#[fixture]
fn timezone_sandbox() -> io::Result<(tempfile::TempDir, Dir)> {
    let sandbox = tempfile::tempdir()?;
    let dir = Dir::open_ambient_dir(sandbox.path(), cap_std::ambient_authority())?;
    Ok((sandbox, dir))
}

#[test]
fn successful_probe_does_not_repair_the_installation() -> Result<()> {
    let repairs = Cell::new(0);
    postgres_timezone::ensure_with(
        || Ok(ProbeOutcome::Ready),
        || {
            repairs.set(repairs.get() + 1);
            Ok(())
        },
    )?;
    assert_eq!(repairs.get(), 0);
    Ok(())
}

#[test]
fn only_invalid_parameter_value_is_a_repairable_set_error() {
    assert!(
        postgres_timezone::is_unsupported_timezone(Some(&SqlState::INVALID_PARAMETER_VALUE)),
        "invalid UTC timezone must be eligible for repair"
    );
    assert!(
        !postgres_timezone::is_unsupported_timezone(Some(&SqlState::INVALID_PASSWORD)),
        "authentication errors must not be treated as unsupported UTC"
    );
    assert!(
        !postgres_timezone::is_unsupported_timezone(None),
        "errors without SQLSTATE must not be treated as unsupported UTC"
    );
}

#[test]
fn failed_probe_does_not_trigger_repair() {
    let repairs = Cell::new(0);
    let error = postgres_timezone::ensure_with(
        || Err(eyre!("connection refused")),
        || {
            repairs.set(repairs.get() + 1);
            Ok(())
        },
    )
    .expect_err("connection errors must be returned");
    assert!(
        format!("{error:#}").contains("connection refused"),
        "original failure must be retained"
    );
    assert_eq!(repairs.get(), 0);
}

#[rstest]
fn unsupported_utc_repairs_bundled_data_and_reprobes(
    timezone_sandbox: io::Result<(tempfile::TempDir, Dir)>,
) -> Result<()> {
    let (_sandbox, timezone_dir) = timezone_sandbox?;
    timezone_dir.write("UCT", b"zero-offset timezone data")?;
    let probes = Cell::new(0);

    postgres_timezone::ensure_with(
        || {
            probes.set(probes.get() + 1);
            if probes.get() == 1 {
                return Ok(ProbeOutcome::Unsupported(eyre!("UTC is unsupported")));
            }
            assert_eq!(timezone_dir.read("UTC")?, b"zero-offset timezone data");
            Ok(ProbeOutcome::Ready)
        },
        || postgres_timezone::ensure_utc_alias(&timezone_dir).map_err(Into::into),
    )?;
    assert_eq!(probes.get(), 2);
    Ok(())
}

#[test]
fn failed_repair_preserves_the_original_probe_failure() {
    let error = postgres_timezone::ensure_with(
        || Ok(ProbeOutcome::Unsupported(eyre!("UTC is unsupported"))),
        || Err(eyre!("bundled metadata is unavailable")),
    )
    .expect_err("repair failure must be returned");
    let message = format!("{error:#}");
    assert!(
        message.contains("UTC is unsupported"),
        "original SQL failure must be retained"
    );
    assert!(
        message.contains("bundled metadata is unavailable"),
        "repair failure must be explained"
    );
}

#[test]
fn metadata_must_describe_a_bundled_path_inside_the_installation() -> Result<()> {
    let installation = Path::new("/embedded/postgres");
    assert_eq!(
        postgres_timezone::bundled_share_path(
            installation,
            "'--enable-debug'",
            "/embedded/postgres/share"
        )?,
        Path::new("share")
    );
    for (configure, sharedir) in [
        ("", "/embedded/postgres/share"),
        (
            "'--with-system-tzdata=/usr/share/zoneinfo'",
            "/embedded/postgres/share",
        ),
        ("--enable-debug", "/usr/share/postgresql"),
        ("--enable-debug", "/embedded/postgres/../outside"),
        ("--enable-debug", "share"),
    ] {
        assert!(
            postgres_timezone::bundled_share_path(installation, configure, sharedir).is_err(),
            "unsafe or invalid metadata must not select a repair path: {configure:?}, {sharedir:?}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn probe_returns_connection_error_when_called_inside_a_tokio_runtime() {
    let error = postgres_timezone::ensure_utc_ready(
        "host=127.0.0.1 port=0 user=test dbname=postgres connect_timeout=1",
        Path::new("/nonexistent-postgresql-installation"),
    )
    .expect_err("an unavailable server must report its connection failure");
    assert!(
        format!("{error:#}").contains("connect for PostgreSQL UTC probe"),
        "the connection failure must be preserved rather than a nested-runtime panic"
    );
}

#[test]
fn invalid_connection_configuration_reports_the_parse_failure() {
    let error = postgres_timezone::ensure_utc_ready(
        "invalid connection string",
        Path::new("/nonexistent-postgresql-installation"),
    )
    .expect_err("invalid connection configuration must fail before repair");
    assert!(
        format!("{error:#}").contains("parse connection for PostgreSQL UTC probe"),
        "the connection configuration error must retain its context"
    );
}

#[rstest]
fn creates_utc_alias_from_the_bundled_uct_timezone(
    timezone_sandbox: io::Result<(tempfile::TempDir, Dir)>,
) -> io::Result<()> {
    let (_sandbox, timezone_dir) = timezone_sandbox?;
    timezone_dir.write("UCT", b"zero-offset timezone data")?;
    postgres_timezone::ensure_utc_alias(&timezone_dir)?;
    postgres_timezone::ensure_utc_alias(&timezone_dir)?;
    assert_eq!(timezone_dir.read("UTC")?, b"zero-offset timezone data");
    Ok(())
}

#[rstest]
fn preserves_an_existing_utc_timezone_file(
    timezone_sandbox: io::Result<(tempfile::TempDir, Dir)>,
) -> io::Result<()> {
    let (_sandbox, timezone_dir) = timezone_sandbox?;
    timezone_dir.write("UCT", b"UCT data")?;
    timezone_dir.write("UTC", b"existing UTC data")?;
    postgres_timezone::ensure_utc_alias(&timezone_dir)?;
    assert_eq!(timezone_dir.read("UTC")?, b"existing UTC data");
    Ok(())
}

#[rstest]
fn reports_when_neither_timezone_name_is_available(
    timezone_sandbox: io::Result<(tempfile::TempDir, Dir)>,
) -> io::Result<()> {
    let (_sandbox, timezone_dir) = timezone_sandbox?;
    let error = postgres_timezone::ensure_utc_alias(&timezone_dir)
        .expect_err("missing UTC and UCT data must stop test setup");
    assert_eq!(error.kind(), io::ErrorKind::NotFound);
    Ok(())
}
