//! Contract coverage for the environment-access policy (issue #464).
//!
//! Three mechanisms have to hold together for the policy to bite:
//!
//! 1. `clippy.toml` lists every prohibited `std::env` API.
//! 2. `clippy::disallowed_methods` is denied, and every workspace package
//!    inherits or restates that deny, so the entries are hard errors.
//! 3. The embedded PostgreSQL settings that test support used to write into
//!    the process are composed by the runner instead, in the `test-rust`
//!    Make recipe.
//!
//! Each assertion matches the mechanism rather than prose about it: the lint
//! entries are read from the parsed table, the deny is read from the parsed
//! lints table, and the Make recipe assertion matches the assignments on the
//! `cargo nextest run` command line.
//!
//! # Mutation proof
//!
//! Mutation-tested on 2026-09-06. Each test guards a different file, so the
//! three mutations below were applied together and each test failed on its own
//! mechanism:
//!
//! - deleting the `std::env::var_os` entry from `clippy.toml` failed
//!   `clippy_configuration_disallows_every_ambient_environment_api` with
//!   `clippy.toml must disallow std::env::var_os`;
//! - deleting `disallowed_methods = "deny"` from `[workspace.lints.clippy]`
//!   failed `every_workspace_member_denies_disallowed_methods` with
//!   `Cargo.toml must deny clippy::disallowed_methods` and `left: None`;
//! - deleting `PG_PASSWORD=$(PG_PASSWORD)` from the `test-rust` recipe failed
//!   `the_test_recipe_composes_the_embedded_postgres_environment`.
//!
//! With the three mutations reverted all three tests pass.

use std::error::Error as StdError;
use std::path::Path;

use cap_std::ambient_authority;
use cap_std::fs::Dir;
use toml::Value;

type TestResult<T = ()> = Result<T, Box<dyn StdError>>;

/// Every `std::env` API the policy prohibits.
const PROHIBITED_ENVIRONMENT_APIS: [&str; 6] = [
    "std::env::var",
    "std::env::var_os",
    "std::env::vars",
    "std::env::vars_os",
    "std::env::set_var",
    "std::env::remove_var",
];

/// Manifests that must make `clippy::disallowed_methods` a hard error.
///
/// `backend` and `architecture-lint` restate the deny locally because they do
/// not yet inherit the shared lint baseline (#461/#462); the remaining members
/// inherit it from `[workspace.lints.clippy]`.
const MANIFESTS_DENYING_DISALLOWED_METHODS: [&str; 5] = [
    "Cargo.toml",
    "backend/Cargo.toml",
    "backend/crates/pagination/Cargo.toml",
    "crates/example-data/Cargo.toml",
    "tools/architecture-lint/Cargo.toml",
];

/// Open a capability handle to the workspace root.
fn workspace_dir() -> TestResult<Dir> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("the backend manifest directory must have a parent")?;
    Ok(Dir::open_ambient_dir(root, ambient_authority())?)
}

/// Read and parse a TOML file relative to the workspace root.
fn read_toml(workspace: &Dir, relative: &str) -> TestResult<Value> {
    let contents = workspace.read_to_string(relative)?;
    Ok(toml::from_str::<Value>(&contents)?)
}

/// Return the `disallowed-methods` paths declared in `clippy.toml`.
fn disallowed_method_paths(workspace: &Dir) -> TestResult<Vec<String>> {
    let policy = read_toml(workspace, "clippy.toml")?;
    let methods = policy
        .get("disallowed-methods")
        .and_then(Value::as_array)
        .ok_or("clippy.toml must declare disallowed-methods")?;
    Ok(methods
        .iter()
        .filter_map(|entry| entry.get("path").and_then(Value::as_str))
        .map(str::to_owned)
        .collect())
}

/// Return the level the workspace assigns to `clippy::disallowed_methods`.
fn workspace_disallowed_methods_level(workspace: &Dir) -> TestResult<Option<String>> {
    let document = read_toml(workspace, "Cargo.toml")?;
    let level = document
        .get("workspace")
        .and_then(|table| table.get("lints"))
        .and_then(|lints| lints.get("clippy"))
        .and_then(|clippy| clippy.get("disallowed_methods"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    Ok(level)
}

/// Return the level a manifest resolves for `clippy::disallowed_methods`.
///
/// A package-level entry wins; otherwise a package that inherits the shared
/// baseline (`[lints] workspace = true`) resolves to the workspace level, as
/// does the workspace manifest itself.
fn disallowed_methods_level(workspace: &Dir, manifest: &str) -> TestResult<Option<String>> {
    let document = read_toml(workspace, manifest)?;
    let package_level = document
        .get("lints")
        .and_then(|lints| lints.get("clippy"))
        .and_then(|clippy| clippy.get("disallowed_methods"))
        .and_then(Value::as_str)
        .map(str::to_owned);
    if package_level.is_some() {
        return Ok(package_level);
    }

    let inherits_workspace = document
        .get("lints")
        .and_then(|lints| lints.get("workspace"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if inherits_workspace || manifest == "Cargo.toml" {
        return workspace_disallowed_methods_level(workspace);
    }

    Ok(None)
}

/// Scenario: a contributor removes one of the prohibited `std::env` entries
/// from `clippy.toml`.
///
/// Invariant: every one of the six APIs stays listed, so the lint keeps
/// reporting ambient environment access.
#[test]
fn clippy_configuration_disallows_every_ambient_environment_api() -> TestResult {
    let workspace = workspace_dir()?;
    let declared = disallowed_method_paths(&workspace)?;
    for prohibited in PROHIBITED_ENVIRONMENT_APIS {
        assert!(
            declared.iter().any(|path| path == prohibited),
            "clippy.toml must disallow {prohibited}, found {declared:?}"
        );
    }
    Ok(())
}

/// Scenario: a contributor drops the deny, leaving the `clippy.toml` entries
/// as warnings a build can ignore.
///
/// Invariant: the workspace and every member manifest still resolve
/// `clippy::disallowed_methods` to `deny`.
#[test]
fn every_workspace_member_denies_disallowed_methods() -> TestResult {
    let workspace = workspace_dir()?;
    for manifest in MANIFESTS_DENYING_DISALLOWED_METHODS {
        assert_eq!(
            disallowed_methods_level(&workspace, manifest)?.as_deref(),
            Some("deny"),
            "{manifest} must deny clippy::disallowed_methods"
        );
    }
    Ok(())
}

/// Scenario: the embedded PostgreSQL settings stop being passed to the test
/// runner, so nothing supplies them once test support no longer writes them.
///
/// Invariant: the `test-rust` recipe still assigns both variables on the
/// `cargo nextest run` command line, and both keep their stable defaults.
#[test]
fn the_test_recipe_composes_the_embedded_postgres_environment() -> TestResult {
    let workspace = workspace_dir()?;
    let makefile = workspace.read_to_string("Makefile")?;

    for default in [
        "PG_PASSWORD ?= wildside_embedded_test",
        "POSTGRESQL_RELEASES_URL ?= https://github.com/theseus-rs/postgresql-binaries",
    ] {
        assert!(
            makefile.contains(default),
            "the Makefile must declare the overridable default `{default}`"
        );
    }

    let recipe = makefile
        .lines()
        .find(|line| line.contains("cargo nextest run --workspace"))
        .ok_or("test-rust must run the workspace suite through cargo nextest")?;
    for assignment in [
        "PG_PASSWORD=$(PG_PASSWORD)",
        "POSTGRESQL_RELEASES_URL=$(POSTGRESQL_RELEASES_URL)",
    ] {
        assert!(
            recipe.contains(assignment),
            "the nextest invocation must carry `{assignment}`; got `{recipe}`"
        );
    }
    Ok(())
}
