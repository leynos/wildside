//! Unit tests for resolving embedded-cluster repair paths through an
//! injected environment reader.

use super::*;
use rstest::rstest;
use std::collections::HashMap;

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

/// Scenario: `PG_VERSION_REQ` is absent, pins a major, or names a range.
///
/// Invariant: only an exact requirement gives the repair a version to enforce;
/// an absent, ranged, or unparsable requirement leaves retained data alone,
/// because a range accepts more than one stored major version.
#[rstest]
#[case::absent(&[], None)]
#[case::pinned(&[("PG_VERSION_REQ", "=16.10.0")], Some(16))]
#[case::bare_pin(&[("PG_VERSION_REQ", "17")], Some(17))]
#[case::lower_bound(&[("PG_VERSION_REQ", ">=17")], None)]
#[case::upper_bound(&[("PG_VERSION_REQ", "<17")], None)]
#[case::unparsable(&[("PG_VERSION_REQ", "latest")], None)]
fn resolve_reads_the_expected_major_version(
    #[case] vars: &'static [(&'static str, &'static str)],
    #[case] expected: Option<u32>,
) {
    let paths = PasswordStatePaths::resolve(stub_env(vars));

    assert_eq!(
        paths.expected_major_version, expected,
        "the configured requirement decides which major version is enforced"
    );
}
