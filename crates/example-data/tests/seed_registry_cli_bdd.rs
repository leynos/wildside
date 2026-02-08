//! Behavioural tests for the seed registry CLI.
//!
//! These scenarios validate that the CLI updates seed registries safely and
//! reports failures for invalid inputs.

#![expect(
    clippy::expect_used,
    reason = "test code uses expect for clear failure messages"
)]

mod test_support;

use camino::{Utf8Path, Utf8PathBuf};
use test_support::{open_registry_dir, unique_temp_path};

use example_data::SeedRegistry;
use example_data::seed_registry_cli::{
    ParseOutcome, Update, apply_update, parse_args, seed_name_for_seed, success_message,
};
use rstest::fixture;
use rstest_bdd::Slot;
use rstest_bdd_macros::{ScenarioState, given, scenario, then, when};

const VALID_REGISTRY_JSON: &str = r#"{
    "version": 1,
    "interestThemeIds": [
        "3fa85f64-5717-4562-b3fc-2c963f66afa6",
        "4fa85f64-5717-4562-b3fc-2c963f66afa7"
    ],
    "safetyToggleIds": [
        "7fa85f64-5717-4562-b3fc-2c963f66afa6"
    ],
    "seeds": [
        {"name": "mossy-owl", "seed": 2026, "userCount": 12}
    ]
}"#;

#[derive(Default, ScenarioState)]
struct World {
    registry_path: Slot<Utf8PathBuf>,
    command_result: Slot<CommandResult>,
    seed_value: Slot<u64>,
}

#[derive(Debug, Clone)]
struct CommandResult {
    is_success: bool,
    stdout: String,
    stderr: String,
    update: Option<Update>,
}

#[fixture]
fn world() -> World {
    World::default()
}

#[given("a seed registry file")]
fn a_seed_registry_file(world: &World) {
    let path = write_registry(VALID_REGISTRY_JSON);
    world.registry_path.set(path);
}

#[given("a seed registry file with seed named \"{name}\"")]
fn a_seed_registry_file_with_seed_named(world: &World, name: String) {
    let json = registry_json_with_seed(&name);
    let path = write_registry(&json);
    world.registry_path.set(path);
}

#[given("an invalid seed registry file")]
fn an_invalid_seed_registry_file(world: &World) {
    let path = write_registry("not json");
    world.registry_path.set(path);
}

#[when("the seed registry CLI adds a seed using RNG value {seed:u64}")]
fn the_seed_registry_cli_adds_a_seed_using_rng_value(world: &World, seed: u64) {
    world.seed_value.set(seed);
    let path = registry_path(world);
    let result = run_cli(&path, &["--seed", &seed.to_string()]);
    world.command_result.set(result);
}

#[when("the seed registry CLI adds a seed named \"{name}\"")]
fn the_seed_registry_cli_adds_a_seed_named(world: &World, name: String) {
    let path = registry_path(world);
    let result = run_cli(&path, &["--name", &name]);
    world.command_result.set(result);
}

#[then("the registry contains the generated seed name")]
fn the_registry_contains_the_generated_seed_name(world: &World) {
    let path = registry_path(world);
    let seed = world.seed_value.get().expect("seed should be set");
    let expected = seed_name_from_value(seed);
    let dir = open_registry_dir(&path).expect("open registry dir");
    let file_name = Utf8Path::new(path.file_name().expect("registry file name"));
    let registry = SeedRegistry::from_file(&dir, file_name).expect("registry should load");

    assert!(registry.find_seed(&expected).is_ok());
}

#[then("the registry contains seed named \"{name}\"")]
fn the_registry_contains_seed_named(world: &World, name: String) {
    let path = registry_path(world);
    let dir = open_registry_dir(&path).expect("open registry dir");
    let file_name = Utf8Path::new(path.file_name().expect("registry file name"));
    let registry = SeedRegistry::from_file(&dir, file_name).expect("registry should load");

    assert!(registry.find_seed(&name).is_ok());
}

#[then("the CLI reports success")]
fn the_cli_reports_success(world: &World) {
    let result = world.command_result.get().expect("command result set");

    assert!(result.is_success, "stderr was: {}", result.stderr);
    let update = result.update.as_ref().expect("update should be recorded");
    let path = registry_path(world);
    let expected = success_message(update, &path);

    assert_eq!(
        result.stdout.trim_end(),
        expected,
        "stdout mismatch: {}",
        result.stdout
    );

    let dir = open_registry_dir(&path).expect("open registry dir");
    let file_name = Utf8Path::new(path.file_name().expect("registry file name"));
    let registry = SeedRegistry::from_file(&dir, file_name).expect("registry should load");
    let seed = registry
        .find_seed(&update.name)
        .expect("registry should contain the new seed");
    assert_eq!(seed.seed(), update.seed);
    assert_eq!(seed.user_count(), update.user_count);
}

#[then("the CLI reports a duplicate seed error")]
fn the_cli_reports_a_duplicate_seed_error(world: &World) {
    let result = world.command_result.get().expect("command result set");

    assert!(!result.is_success);
    assert!(
        result.stderr.contains("already exists in registry"),
        "stderr did not mention a duplicate seed: {}",
        result.stderr
    );
    assert!(
        result.stderr.contains("mossy-owl"),
        "stderr did not include the duplicate seed name: {}",
        result.stderr
    );
}

#[then("the registry remains unchanged")]
fn the_registry_remains_unchanged(world: &World) {
    let path = registry_path(world);
    let dir = open_registry_dir(&path).expect("open registry dir");
    let file_name = Utf8Path::new(path.file_name().expect("registry file name"));
    let registry = SeedRegistry::from_file(&dir, file_name).expect("registry should load");

    assert_eq!(registry.seeds().len(), 1);
    assert!(registry.find_seed("mossy-owl").is_ok());
}

#[then("the CLI reports a registry parse error")]
fn the_cli_reports_a_registry_parse_error(world: &World) {
    let result = world.command_result.get().expect("command result set");

    assert!(!result.is_success);
    assert!(result.stderr.contains("invalid registry JSON"));
}

#[scenario(path = "tests/features/seed_registry_cli.feature", index = 0)]
fn add_seed_with_generated_name(world: World) {
    drop(world);
}

#[scenario(path = "tests/features/seed_registry_cli.feature", index = 1)]
fn add_seed_with_explicit_name(world: World) {
    drop(world);
}

#[scenario(path = "tests/features/seed_registry_cli.feature", index = 2)]
fn reject_duplicate_seed_name(world: World) {
    drop(world);
}

#[scenario(path = "tests/features/seed_registry_cli.feature", index = 3)]
fn reject_invalid_registry_json(world: World) {
    drop(world);
}

fn registry_path(world: &World) -> Utf8PathBuf {
    world
        .registry_path
        .get()
        .expect("registry path should be set")
        .clone()
}

fn run_cli(registry_path: &Utf8Path, extra_args: &[&str]) -> CommandResult {
    let mut args = vec!["--registry".to_owned(), registry_path.as_str().to_owned()];
    args.extend(extra_args.iter().map(std::string::ToString::to_string));

    let parse_result = match parse_args(args.into_iter()) {
        Ok(outcome) => outcome,
        Err(err) => {
            return CommandResult {
                is_success: false,
                stdout: String::new(),
                stderr: err.to_string(),
                update: None,
            };
        }
    };

    let ParseOutcome::Options(options) = parse_result else {
        return CommandResult {
            is_success: false,
            stdout: String::new(),
            stderr: "unexpected help output".to_owned(),
            update: None,
        };
    };

    match apply_update(&options) {
        Ok(update) => CommandResult {
            is_success: true,
            stdout: success_message(&update, options.registry_path()),
            stderr: String::new(),
            update: Some(update),
        },
        Err(err) => CommandResult {
            is_success: false,
            stdout: String::new(),
            stderr: err.to_string(),
            update: None,
        },
    }
}

fn seed_name_from_value(seed: u64) -> String {
    seed_name_for_seed(seed).expect("seed name should be generated")
}

fn write_registry(contents: &str) -> Utf8PathBuf {
    let path = temp_registry_path();
    let dir = open_registry_dir(&path).expect("open registry dir");
    let file_name = path.file_name().expect("registry file name");
    dir.write(file_name, contents).expect("write registry file");
    path
}

fn registry_json_with_seed(name: &str) -> String {
    format!(
        r#"{{
    "version": 1,
    "interestThemeIds": ["3fa85f64-5717-4562-b3fc-2c963f66afa6"],
    "safetyToggleIds": ["7fa85f64-5717-4562-b3fc-2c963f66afa6"],
    "seeds": [{{"name": "{name}", "seed": 2026, "userCount": 12}}]
}}"#
    )
}

fn temp_registry_path() -> Utf8PathBuf {
    unique_temp_path("seed-registry-cli", "seeds.json").expect("create temp registry path")
}
