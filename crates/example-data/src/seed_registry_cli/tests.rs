//! Unit tests for the seed registry CLI helpers.

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs::Dir};
use rstest::{fixture, rstest};
use std::sync::atomic::{AtomicUsize, Ordering};

struct RegistryFixture {
    path: Utf8PathBuf,
}

impl RegistryFixture {
    fn path(&self) -> Utf8PathBuf {
        self.path.clone()
    }

    fn load(&self) -> SeedRegistry {
        let dir = open_registry_dir(&self.path);
        let file_name = Utf8Path::new(self.path.file_name().expect("registry file name"));
        SeedRegistry::from_file(&dir, file_name).expect("load registry")
    }
}

impl Drop for RegistryFixture {
    fn drop(&mut self) {
        cleanup_path(&self.path);
    }
}

use super::*;
use crate::error::RegistryError;

const VALID_JSON: &str = r#"{
    "version": 1,
    "interestThemeIds": ["3fa85f64-5717-4562-b3fc-2c963f66afa6"],
    "safetyToggleIds": [],
    "seeds": [{"name": "mossy-owl", "seed": 2026, "userCount": 12}]
}"#;

#[fixture]
fn registry_fixture() -> RegistryFixture {
    RegistryFixture {
        path: write_registry(VALID_JSON),
    }
}

#[test]
fn parse_args_returns_help_for_help_flag() {
    let args = vec!["--help".to_owned()];

    let outcome = parse_args(args.into_iter()).expect("parse args");

    assert!(matches!(outcome, ParseOutcome::Help));
}

#[test]
fn parse_args_requires_registry_path() {
    let args = vec!["--seed".to_owned(), "42".to_owned()];

    let err = parse_args(args.into_iter()).expect_err("expected error");

    assert_eq!(err, CliError::MissingRegistryPath);
}

#[rstest]
#[case("--registry")]
#[case("--seed")]
#[case("--name")]
#[case("--user-count")]
fn parse_args_reports_missing_value(#[case] flag: &'static str) {
    let args = vec![flag.to_owned()];

    let err = parse_args(args.into_iter()).expect_err("expected error");

    assert_eq!(err, CliError::MissingValue { flag });
}

#[test]
fn parse_args_reports_unknown_arguments() {
    let args = vec![
        "--registry".to_owned(),
        "seeds.json".to_owned(),
        "--nope".to_owned(),
    ];

    let err = parse_args(args.into_iter()).expect_err("expected error");

    assert_eq!(
        err,
        CliError::UnknownArgument {
            value: "--nope".to_owned(),
        }
    );
}

#[test]
fn parse_args_reports_invalid_numbers() {
    let args = vec![
        "--registry".to_owned(),
        "seeds.json".to_owned(),
        "--seed".to_owned(),
        "not-a-number".to_owned(),
    ];

    let err = parse_args(args.into_iter()).expect_err("expected error");

    let CliError::InvalidNumber { flag, value, .. } = err else {
        panic!("expected invalid number error");
    };

    assert_eq!(flag, "--seed");
    assert_eq!(value, "not-a-number");
}

#[test]
fn parse_args_parses_full_options() {
    let args = vec![
        "--registry".to_owned(),
        "seeds.json".to_owned(),
        "--seed".to_owned(),
        "2026".to_owned(),
        "--name".to_owned(),
        "river-stone".to_owned(),
        "--user-count".to_owned(),
        "9".to_owned(),
    ];

    let ParseOutcome::Options(options) = parse_args(args.into_iter()).expect("parse args") else {
        panic!("expected options");
    };

    assert_eq!(options.registry_path, Utf8PathBuf::from("seeds.json"));
    assert_eq!(options.seed, Some(2026));
    assert_eq!(options.name.as_deref(), Some("river-stone"));
    assert_eq!(options.user_count, Some(9));
}

#[rstest]
fn apply_update_appends_explicit_seed(registry_fixture: RegistryFixture) {
    let path = registry_fixture.path();
    let options = Options {
        registry_path: path.clone(),
        seed: Some(808),
        name: Some("river-stone".to_owned()),
        user_count: Some(4),
    };

    let update = apply_update(&options).expect("apply update");

    assert_eq!(
        update,
        Update {
            name: "river-stone".to_owned(),
            seed: 808,
            user_count: 4,
        }
    );

    let registry = registry_fixture.load();
    assert!(registry.find_seed("river-stone").is_ok());
}

#[rstest]
fn apply_update_generates_name_from_seed(registry_fixture: RegistryFixture) {
    let path = registry_fixture.path();
    let options = Options {
        registry_path: path.clone(),
        seed: Some(2026),
        name: None,
        user_count: None,
    };

    let update = apply_update(&options).expect("apply update");
    let expected_name = seed_name_for_seed(2026).expect("seed name");

    assert_eq!(update.name, expected_name);
    assert_eq!(update.seed, 2026);
    assert_eq!(update.user_count, DEFAULT_USER_COUNT);

    let registry = registry_fixture.load();
    assert!(registry.find_seed(&update.name).is_ok());
}

#[rstest]
fn apply_update_reports_duplicate_explicit_name(registry_fixture: RegistryFixture) {
    let path = registry_fixture.path();
    let options = Options {
        registry_path: path.clone(),
        seed: Some(404),
        name: Some("mossy-owl".to_owned()),
        user_count: None,
    };

    let err = apply_update(&options).expect_err("expected duplicate error");

    let CliError::RegistryError { source } = err else {
        panic!("expected registry error");
    };

    assert_eq!(
        source,
        RegistryError::DuplicateSeedName {
            name: "mossy-owl".to_owned(),
        }
    );
}

#[test]
fn apply_update_reports_duplicate_generated_name() {
    let generated_name = seed_name_for_seed(2026).expect("seed name");
    let json = registry_json_with_seed(&generated_name, 2026);
    let path = write_registry(&json);
    let options = Options {
        registry_path: path.clone(),
        seed: Some(2026),
        name: None,
        user_count: None,
    };

    let err = apply_update(&options).expect_err("expected duplicate error");

    assert_eq!(
        err,
        CliError::DuplicateGeneratedName {
            name: generated_name,
        }
    );
}

#[test]
fn apply_update_reports_registry_io_errors() {
    let path = unique_temp_path("missing.json");
    let file_name = Utf8PathBuf::from(path.file_name().expect("registry file name"));
    let options = Options {
        registry_path: path.clone(),
        seed: Some(1),
        name: Some("river-stone".to_owned()),
        user_count: None,
    };

    let err = apply_update(&options).expect_err("expected error");

    let CliError::RegistryError { source } = err else {
        panic!("expected registry error");
    };

    match source {
        RegistryError::IoError { path: err_path, .. } => {
            assert_eq!(err_path, file_name);
        }
        _ => panic!("expected IO error"),
    }

    cleanup_path(&path);
}

#[test]
fn apply_update_reports_open_registry_dir_errors() {
    let path = unique_missing_path("seeds.json");
    let options = Options {
        registry_path: path.clone(),
        seed: Some(1),
        name: Some("river-stone".to_owned()),
        user_count: None,
    };

    let err = apply_update(&options).expect_err("expected error");

    let CliError::RegistryError { source } = err else {
        panic!("expected registry error");
    };

    match source {
        RegistryError::IoError {
            path: err_path,
            message,
        } => {
            assert_eq!(err_path, path);
            assert!(!message.is_empty());
        }
        _ => panic!("expected IO error"),
    }
}

#[test]
fn success_message_formats_expected_output() {
    let update = Update {
        name: "mossy-owl".to_owned(),
        seed: 2026,
        user_count: 12,
    };

    let message = success_message(&update, Utf8Path::new("seeds.json"));

    assert_eq!(
        message,
        "Added seed \"mossy-owl\" (seed=2026, userCount=12) to seeds.json"
    );
}

fn registry_json_with_seed(name: &str, seed: u64) -> String {
    format!(
        r#"{{
    "version": 1,
    "interestThemeIds": ["3fa85f64-5717-4562-b3fc-2c963f66afa6"],
    "safetyToggleIds": [],
    "seeds": [{{"name": "{name}", "seed": {seed}, "userCount": 12}}]
}}"#
    )
}

fn write_registry(json: &str) -> Utf8PathBuf {
    let path = unique_temp_path("seeds.json");
    let dir = open_registry_dir(&path);
    let file_name = path.file_name().expect("registry file name");
    dir.write(file_name, json).expect("write registry");
    path
}

fn cleanup_path(path: &Utf8Path) {
    if let Some(parent) = path.parent() {
        let root = Dir::open_ambient_dir(".", ambient_authority()).expect("open workspace dir");
        drop(root.remove_dir_all(parent));
    }
}

fn unique_temp_path(file_name: &str) -> Utf8PathBuf {
    static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let process_id = std::process::id();
    let dir_name = format!("seed-registry-cli-{process_id}-{counter}");
    let dir = Utf8PathBuf::from("target")
        .join("example-data-tests")
        .join(dir_name);
    let root = Dir::open_ambient_dir(".", ambient_authority()).expect("open workspace dir");
    root.create_dir_all(&dir).expect("create temp dir");
    dir.join(file_name)
}

fn unique_missing_path(file_name: &str) -> Utf8PathBuf {
    static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir_name = format!("seed-registry-cli-missing-{counter}");
    Utf8PathBuf::from("target")
        .join("example-data-tests")
        .join(dir_name)
        .join(file_name)
}

fn open_registry_dir(path: &Utf8Path) -> Dir {
    let parent = path.parent().unwrap_or_else(|| Utf8Path::new("."));
    Dir::open_ambient_dir(parent, ambient_authority()).expect("open registry dir")
}
