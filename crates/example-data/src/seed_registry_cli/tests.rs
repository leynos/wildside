//! Unit tests for the seed registry CLI helpers.

#[path = "../../tests/test_support.rs"]
mod test_support;

use camino::{Utf8Path, Utf8PathBuf};
use rstest::{fixture, rstest};
use test_support::{cleanup_path, open_registry_dir, unique_missing_path, unique_temp_path};

type TestResult = Result<(), Box<dyn std::error::Error>>;
type FixtureResult = Result<RegistryFixture, Box<dyn std::error::Error>>;

struct RegistryFixture {
    path: Utf8PathBuf,
}

impl RegistryFixture {
    fn path(&self) -> Utf8PathBuf {
        self.path.clone()
    }

    fn load(&self) -> Result<SeedRegistry, Box<dyn std::error::Error>> {
        let dir = open_registry_dir(&self.path)?;
        let file_name = Utf8Path::new(
            self.path
                .file_name()
                .ok_or("registry path missing file name")?,
        );
        Ok(SeedRegistry::from_file(&dir, file_name)?)
    }
}

impl Drop for RegistryFixture {
    fn drop(&mut self) {
        // Best-effort cleanup; Drop cannot propagate errors.
        drop(cleanup_path(&self.path));
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

fn check_eq<T: PartialEq + std::fmt::Debug>(actual: &T, expected: &T, context: &str) -> TestResult {
    if actual == expected {
        Ok(())
    } else {
        Err(format!("{context}: expected {expected:?}, got {actual:?}").into())
    }
}

fn check<T, E: std::fmt::Debug>(result: Result<T, E>, context: &str) -> TestResult {
    match result {
        Ok(_) => Ok(()),
        Err(err) => Err(format!("{context}: {err:?}").into()),
    }
}

#[fixture]
fn registry_fixture() -> FixtureResult {
    Ok(RegistryFixture {
        path: write_registry(VALID_JSON)?,
    })
}

#[test]
fn parse_args_returns_help_for_help_flag() -> TestResult {
    let args = vec!["--help".to_owned()];
    let outcome = parse_args(args.into_iter())?;
    if !matches!(outcome, ParseOutcome::Help) {
        return Err(format!("expected Help outcome, got {outcome:?}").into());
    }
    Ok(())
}

#[test]
fn parse_args_requires_registry_path() -> TestResult {
    let args = vec!["--seed".to_owned(), "42".to_owned()];
    let err = parse_args(args.into_iter())
        .err()
        .ok_or("expected parse_args to fail")?;
    check_eq(&err, &CliError::MissingRegistryPath, "parse_args error")
}

#[rstest]
#[case("--registry")]
#[case("--seed")]
#[case("--name")]
#[case("--user-count")]
fn parse_args_reports_missing_value(#[case] flag: &'static str) -> TestResult {
    let args = vec![flag.to_owned()];
    let err = parse_args(args.into_iter())
        .err()
        .ok_or("expected parse_args to fail")?;
    check_eq(&err, &CliError::MissingValue { flag }, "parse_args error")
}

#[test]
fn parse_args_reports_unknown_arguments() -> TestResult {
    let args = vec![
        "--registry".to_owned(),
        "seeds.json".to_owned(),
        "--nope".to_owned(),
    ];
    let err = parse_args(args.into_iter())
        .err()
        .ok_or("expected parse_args to fail")?;
    check_eq(
        &err,
        &CliError::UnknownArgument {
            value: "--nope".to_owned(),
        },
        "parse_args error",
    )
}

#[test]
fn parse_args_reports_invalid_numbers() -> TestResult {
    let args = vec![
        "--registry".to_owned(),
        "seeds.json".to_owned(),
        "--seed".to_owned(),
        "not-a-number".to_owned(),
    ];
    let err = parse_args(args.into_iter())
        .err()
        .ok_or("expected parse_args to fail")?;
    let CliError::InvalidNumber { flag, value, .. } = err else {
        return Err("expected invalid number error".into());
    };
    check_eq(&flag, &"--seed", "flag")?;
    check_eq(&value, &"not-a-number".to_owned(), "value")
}

#[test]
fn parse_args_parses_full_options() -> TestResult {
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
    let ParseOutcome::Options(options) = parse_args(args.into_iter())? else {
        return Err("expected options".into());
    };
    check_eq(
        &options.registry_path,
        &Utf8PathBuf::from("seeds.json"),
        "registry_path",
    )?;
    check_eq(&options.seed, &Some(2026), "seed")?;
    check_eq(&options.name.as_deref(), &Some("river-stone"), "name")?;
    check_eq(&options.user_count, &Some(9), "user_count")
}

#[rstest]
fn apply_update_appends_explicit_seed(registry_fixture: FixtureResult) -> TestResult {
    let fixture = registry_fixture?;
    let path = fixture.path();
    let options = Options {
        registry_path: path.clone(),
        seed: Some(808),
        name: Some("river-stone".to_owned()),
        user_count: Some(4),
    };
    let update = apply_update(&options)?;
    check_eq(
        &update,
        &Update {
            name: "river-stone".to_owned(),
            seed: 808,
            user_count: 4,
        },
        "update",
    )?;
    let registry = fixture.load()?;
    check(registry.find_seed("river-stone"), "find river-stone")
}

#[rstest]
fn apply_update_generates_name_from_seed(registry_fixture: FixtureResult) -> TestResult {
    let fixture = registry_fixture?;
    let path = fixture.path();
    let options = Options {
        registry_path: path.clone(),
        seed: Some(2026),
        name: None,
        user_count: None,
    };
    let update = apply_update(&options)?;
    let expected_name = seed_name_for_seed(2026)?;
    check_eq(&update.name, &expected_name, "update.name")?;
    check_eq(&update.seed, &2026, "update.seed")?;
    check_eq(&update.user_count, &DEFAULT_USER_COUNT, "update.user_count")?;
    let registry = fixture.load()?;
    check(registry.find_seed(&update.name), "find generated name")
}

#[rstest]
fn apply_update_reports_duplicate_explicit_name(registry_fixture: FixtureResult) -> TestResult {
    let fixture = registry_fixture?;
    let path = fixture.path();
    let options = Options {
        registry_path: path.clone(),
        seed: Some(404),
        name: Some("mossy-owl".to_owned()),
        user_count: None,
    };
    let err = apply_update(&options)
        .err()
        .ok_or("expected duplicate error")?;
    let CliError::RegistryError { source } = err else {
        return Err("expected registry error".into());
    };
    check_eq(
        &source,
        &RegistryError::DuplicateSeedName {
            name: "mossy-owl".to_owned(),
        },
        "duplicate error",
    )
}

#[test]
fn apply_update_reports_duplicate_generated_name() -> TestResult {
    let generated_name = seed_name_for_seed(2026)?;
    let json = registry_json_with_seed(&generated_name, 2026);
    let path = write_registry(&json)?;
    let options = Options {
        registry_path: path.clone(),
        seed: Some(2026),
        name: None,
        user_count: None,
    };
    let err = apply_update(&options)
        .err()
        .ok_or("expected duplicate error")?;
    check_eq(
        &err,
        &CliError::DuplicateGeneratedName {
            name: generated_name,
        },
        "duplicate generated error",
    )
}

#[test]
fn apply_update_reports_registry_io_errors() -> TestResult {
    let path = unique_temp_path("seed-registry-cli", "missing.json")?;
    let file_name = Utf8PathBuf::from(path.file_name().ok_or("registry path missing file name")?);
    let options = Options {
        registry_path: path.clone(),
        seed: Some(1),
        name: Some("river-stone".to_owned()),
        user_count: None,
    };
    let err = apply_update(&options).err().ok_or("expected error")?;
    let CliError::RegistryError { source } = err else {
        return Err("expected registry error".into());
    };
    match source {
        RegistryError::IoError { path: err_path, .. } => {
            check_eq(&err_path, &file_name, "io error path")?;
        }
        other => return Err(format!("expected IO error, got {other:?}").into()),
    }
    cleanup_path(&path)?;
    Ok(())
}

#[test]
fn apply_update_reports_open_registry_dir_errors() -> TestResult {
    let path = unique_missing_path("seeds.json");
    let options = Options {
        registry_path: path.clone(),
        seed: Some(1),
        name: Some("river-stone".to_owned()),
        user_count: None,
    };
    let err = apply_update(&options).err().ok_or("expected error")?;
    let CliError::RegistryError { source } = err else {
        return Err("expected registry error".into());
    };
    match source {
        RegistryError::IoError {
            path: err_path,
            message,
        } => {
            check_eq(&err_path, &path, "io error path")?;
            if message.is_empty() {
                return Err("io error message should not be empty".into());
            }
            Ok(())
        }
        other => Err(format!("expected IO error, got {other:?}").into()),
    }
}

#[test]
fn success_message_formats_expected_output() -> TestResult {
    let update = Update {
        name: "mossy-owl".to_owned(),
        seed: 2026,
        user_count: 12,
    };
    let message = success_message(&update, Utf8Path::new("seeds.json"));
    check_eq(
        &message,
        &"Added seed \"mossy-owl\" (seed=2026, userCount=12) to seeds.json".to_owned(),
        "success message",
    )
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

fn write_registry(json: &str) -> Result<Utf8PathBuf, Box<dyn std::error::Error>> {
    let path = unique_temp_path("seed-registry-cli", "seeds.json")?;
    let dir = open_registry_dir(&path)?;
    let file_name = path.file_name().ok_or("registry path missing file name")?;
    dir.write(file_name, json)?;
    Ok(path)
}
