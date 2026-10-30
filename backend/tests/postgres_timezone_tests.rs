//! Tests for embedded PostgreSQL UTC readiness and bundled timezone repair.

#[path = "support/postgres_timezone.rs"]
mod postgres_timezone;

use std::cell::Cell;
use std::io;
use std::path::{Component, Path, PathBuf};

use cap_std::fs::Dir;
use color_eyre::eyre::Result;
use color_eyre::eyre::eyre;
use postgres::error::SqlState;
use postgres_timezone::ProbeOutcome;
use proptest::prelude::*;
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

/// Copy the bundled `UCT` file to `UTC` unless an alias already exists.
///
/// Both cases call `ensure_utc_alias` twice so the idempotent second call is
/// covered alongside the repair.
#[rstest]
#[case::copies_bundled_uct(None, b"UCT data".as_slice())]
#[case::preserves_existing_utc(
    Some(b"existing UTC data".as_slice()),
    b"existing UTC data".as_slice()
)]
fn ensures_utc_alias_from_the_bundled_uct_timezone(
    timezone_sandbox: io::Result<(tempfile::TempDir, Dir)>,
    #[case] existing_utc: Option<&[u8]>,
    #[case] expected: &[u8],
) -> io::Result<()> {
    let (_sandbox, timezone_dir) = timezone_sandbox?;
    timezone_dir.write("UCT", b"UCT data")?;
    if let Some(existing) = existing_utc {
        timezone_dir.write("UTC", existing)?;
    }
    postgres_timezone::ensure_utc_alias(&timezone_dir)?;
    postgres_timezone::ensure_utc_alias(&timezone_dir)?;
    assert_eq!(timezone_dir.read("UTC")?, expected);
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

/// Stand-in `pg_config` that reports the metadata written beside it.
#[cfg(unix)]
const PG_CONFIG_SCRIPT: &str = concat!(
    "#!/bin/sh\n",
    "case \"$1\" in\n",
    "  --configure) cat \"$(dirname \"$0\")/configure.txt\" ;;\n",
    "  --sharedir) cat \"$(dirname \"$0\")/sharedir.txt\" ;;\n",
    "  *) exit 2 ;;\n",
    "esac\n",
);

/// Write a stand-in `bin/pg_config` reporting the supplied metadata.
///
/// The script reads its answers from files beside it, so the repair path can
/// be exercised without a real PostgreSQL build. Writes go through a `Dir`
/// handle to keep the fixture inside the capability-based filesystem policy.
#[cfg(unix)]
fn install_fake_pg_config(installation: &Path, configure: &str, sharedir: &Path) -> io::Result<()> {
    use cap_std::fs::PermissionsExt;

    let dir = Dir::open_ambient_dir(installation, cap_std::ambient_authority())?;
    dir.create_dir_all("bin")?;
    dir.write("bin/configure.txt", configure)?;
    dir.write("bin/sharedir.txt", sharedir.to_string_lossy().as_bytes())?;
    dir.write("bin/pg_config", PG_CONFIG_SCRIPT)?;
    dir.set_permissions("bin/pg_config", cap_std::fs::Permissions::from_mode(0o755))
}

/// Build a throwaway installation with bundled `UCT` timezone data.
///
/// `sharedir` overrides what the stand-in `pg_config` reports; `None` reports
/// the installation's own `share` directory. The bundled `share/timezone/UCT`
/// file is always created so a refused repair can be shown to leave no alias.
#[cfg(unix)]
fn bundled_installation(
    configure: &str,
    sharedir: Option<&Path>,
) -> io::Result<(tempfile::TempDir, PathBuf)> {
    let sandbox = tempfile::tempdir()?;
    let installation = sandbox.path().canonicalize()?;
    let reported = sharedir.map_or_else(|| installation.join("share"), Path::to_path_buf);
    install_fake_pg_config(&installation, configure, &reported)?;
    let dir = Dir::open_ambient_dir(&installation, cap_std::ambient_authority())?;
    dir.create_dir_all("share/timezone")?;
    dir.write("share/timezone/UCT", b"bundled UCT data")?;
    Ok((sandbox, installation))
}

/// Repair runs the installation's `pg_config` and hard-links `UCT` to `UTC`.
#[cfg(unix)]
#[test]
fn repair_links_utc_beside_the_verified_bundled_data() -> Result<()> {
    use cap_std::fs::MetadataExt;

    let (_sandbox, installation) =
        bundled_installation("--prefix=/opt/postgres --enable-debug", None)?;
    let dir = Dir::open_ambient_dir(&installation, cap_std::ambient_authority())?;

    postgres_timezone::repair_bundled_utc(&installation)?;

    assert_eq!(
        dir.read("share/timezone/UTC")?,
        b"bundled UCT data".to_vec()
    );
    let uct = dir.metadata("share/timezone/UCT")?;
    let utc = dir.metadata("share/timezone/UTC")?;
    assert_eq!(
        uct.ino(),
        utc.ino(),
        "UTC must be a hard link to the bundled UCT data"
    );
    Ok(())
}

/// Repair refuses metadata that names a system or outside share directory.
#[cfg(unix)]
#[test]
fn repair_refuses_metadata_outside_the_installation() -> Result<()> {
    let outside = tempfile::tempdir()?;
    let outside = outside.path().canonicalize()?;
    let cases = [
        (
            "--prefix=/opt/postgres --with-system-tzdata=/usr/share/zoneinfo",
            None,
            "uses system timezone data",
        ),
        (
            "--prefix=/opt/postgres",
            Some(outside.as_path()),
            "lies outside its installation",
        ),
        (
            "--prefix=/opt/postgres",
            Some(Path::new("share")),
            "non-absolute share directory",
        ),
        ("not configure metadata", None, "invalid configure metadata"),
    ];

    for (configure, sharedir, expected) in cases {
        let (_sandbox, installation) = bundled_installation(configure, sharedir)?;
        let dir = Dir::open_ambient_dir(&installation, cap_std::ambient_authority())?;

        let error = postgres_timezone::repair_bundled_utc(&installation)
            .expect_err("unsafe metadata must not authorize a repair");
        assert!(
            format!("{error:#}").contains(expected),
            "expected {expected:?} for {configure:?}, got {error:#}"
        );
        assert!(
            !dir.exists("share/timezone/UTC"),
            "a refused repair must not create the UTC alias"
        );
    }
    Ok(())
}

proptest! {
    /// Accepted metadata always yields a path inside the installation.
    #[test]
    fn accepted_share_paths_stay_inside_the_installation(
        segments in prop::collection::vec("[a-z][a-z0-9_.-]{0,11}", 1..5),
        extra_option in prop::option::of("--[a-z-]{1,12}"),
    ) {
        let installation = Path::new("/opt/postgres");
        let relative: PathBuf = segments.iter().collect();
        let sharedir = installation.join(&relative);
        let configure = extra_option.map_or_else(
            || "--prefix=/opt/postgres".to_owned(),
            |option| format!("--prefix=/opt/postgres {option}"),
        );

        let accepted = postgres_timezone::bundled_share_path(
            installation,
            &configure,
            &sharedir.to_string_lossy(),
        )
        .unwrap_or_else(|error| panic!("valid metadata was rejected: {error:#}"));

        prop_assert!(!accepted.is_absolute());
        prop_assert!(
            accepted
                .components()
                .all(|component| matches!(component, Component::Normal(_)))
        );
        prop_assert_eq!(installation.join(&accepted), sharedir);
    }

    /// Unsafe or invalid metadata never authorizes a repair path.
    #[test]
    fn unsafe_share_metadata_is_rejected(
        segments in prop::collection::vec("[a-z][a-z0-9_.-]{0,11}", 1..5),
        suffix in "[a-z][a-z0-9_.-]{0,11}",
    ) {
        let installation = Path::new("/opt/postgres");
        let inside = installation.join(segments.iter().collect::<PathBuf>());
        let mut escaping = inside.clone();
        escaping.push("..");
        escaping.push(&suffix);

        let cases = [
            (String::new(), inside.to_string_lossy().into_owned()),
            (
                "--with-system-tzdata=/usr/share/zoneinfo".to_owned(),
                inside.to_string_lossy().into_owned(),
            ),
            (
                "--prefix=/opt/postgres".to_owned(),
                format!("/usr/share/{suffix}"),
            ),
            (
                "--prefix=/opt/postgres".to_owned(),
                format!("share/{suffix}"),
            ),
            (
                "--prefix=/opt/postgres".to_owned(),
                escaping.to_string_lossy().into_owned(),
            ),
        ];

        for (configure, sharedir) in cases {
            prop_assert!(
                postgres_timezone::bundled_share_path(installation, &configure, &sharedir)
                    .is_err(),
                "unsafe metadata must be rejected: {configure:?}, {sharedir:?}"
            );
        }
    }
}
