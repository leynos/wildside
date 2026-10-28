//! Contract coverage for the environment-access policy's configuration
//! (issue #464).
//!
//! This target asserts what the repository *declares*: the prohibited APIs in
//! `clippy.toml`, the deny that turns them into hard errors in every workspace
//! package, and the embedded PostgreSQL settings the runners compose now that
//! test support no longer writes them. Cargo supplies non-secret version and
//! mirror defaults; Make and CI supply the test password. That the lint
//! actually fires on those entries is proved separately, in
//! `environment_policy_lint.rs`.
//!
//! Every file is read with `include_str!`, so moving or deleting one is a
//! compile failure rather than a runtime surprise, and each assertion matches
//! the mechanism rather than prose about it: the entries and levels come from
//! the parsed tables, and the Make assertion checks the recipe-scoped test
//! environment exports.
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
//! - deleting `export PG_PASSWORD` from the Makefile failed
//!   `the_makefile_composes_the_embedded_postgres_environment`.
//!
//! On 2026-09-07 the Makefile assertions were re-proved against the weaker
//! failure they used to admit: commenting the default out, so the text stays
//! present but Make never applies it, passed the earlier substring search and
//! now fails with `the Makefile must carry
//! PG_PASSWORD ?= wildside_embedded_test as a live directive, not inside a
//! comment`.
//!
//! With the three mutations reverted all three tests pass. On 2026-09-07,
//! separately, deleting `allow_attributes = "deny"` from `backend/Cargo.toml`
//! failed `a_local_deny_carries_the_companion_attribute_lints` with
//! `backend/Cargo.toml denies disallowed_methods locally, so it must deny
//! clippy::allow_attributes too`.

#[path = "environment_policy_contract/export_tests.rs"]
mod export_tests;

use std::error::Error as StdError;

use toml::Value;

type TestResult<T = ()> = Result<T, Box<dyn StdError>>;

const CLIPPY_POLICY: &str = include_str!("../../clippy.toml");
const WORKSPACE_MANIFEST: &str = include_str!("../../Cargo.toml");
const BACKEND_MANIFEST: &str = include_str!("../Cargo.toml");
const PAGINATION_MANIFEST: &str = include_str!("../crates/pagination/Cargo.toml");
const EXAMPLE_DATA_MANIFEST: &str = include_str!("../../crates/example-data/Cargo.toml");
const ARCHITECTURE_LINT_MANIFEST: &str = include_str!("../../tools/architecture-lint/Cargo.toml");
const MAKEFILE: &str = include_str!("../../Makefile");
const CARGO_CONFIG: &str = include_str!("../../.cargo/config.toml");

/// Every `std::env` API the policy prohibits.
const PROHIBITED_ENVIRONMENT_APIS: [&str; 6] = [
    "std::env::var",
    "std::env::var_os",
    "std::env::vars",
    "std::env::vars_os",
    "std::env::set_var",
    "std::env::remove_var",
];

/// A manifest's source and diagnostic name belong to the same test input.
#[derive(Clone, Copy)]
struct Manifest<'a> {
    path: &'a str,
    contents: &'a str,
}

/// Manifests that must make `clippy::disallowed_methods` a hard error.
const MANIFESTS_DENYING_DISALLOWED_METHODS: [Manifest<'_>; 5] = [
    Manifest {
        path: "Cargo.toml",
        contents: WORKSPACE_MANIFEST,
    },
    Manifest {
        path: "backend/Cargo.toml",
        contents: BACKEND_MANIFEST,
    },
    Manifest {
        path: "backend/crates/pagination/Cargo.toml",
        contents: PAGINATION_MANIFEST,
    },
    Manifest {
        path: "crates/example-data/Cargo.toml",
        contents: EXAMPLE_DATA_MANIFEST,
    },
    Manifest {
        path: "tools/architecture-lint/Cargo.toml",
        contents: ARCHITECTURE_LINT_MANIFEST,
    },
];

/// Manifests that restate the deny locally because they do not yet inherit the
/// shared lint baseline (#461/#462).
///
/// A package-level deny is only half the guard: without `allow_attributes` a
/// contributor could silence a prohibited call with a bare `#[allow]`, which
/// never warns once the site is migrated.
const MANIFESTS_WITH_A_LOCAL_DENY: [Manifest<'_>; 2] = [
    Manifest {
        path: "backend/Cargo.toml",
        contents: BACKEND_MANIFEST,
    },
    Manifest {
        path: "tools/architecture-lint/Cargo.toml",
        contents: ARCHITECTURE_LINT_MANIFEST,
    },
];

/// Lints a locally denying package must also deny.
const COMPANION_ATTRIBUTE_LINTS: [&str; 2] =
    ["allow_attributes", "allow_attributes_without_reason"];

/// Return the `disallowed-methods` paths declared in `clippy.toml`.
fn disallowed_method_paths() -> TestResult<Vec<String>> {
    let policy: Value = toml::from_str(CLIPPY_POLICY)?;
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

/// Return the level a manifest table assigns to one Clippy lint.
fn lint_level(document: &Value, lint: &str) -> Option<String> {
    document
        .get("lints")
        .and_then(|lints| lints.get("clippy"))
        .and_then(|clippy| clippy.get(lint))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

/// Return the level a manifest resolves for one Clippy lint.
///
/// A package-level entry wins; otherwise a package that inherits the shared
/// baseline (`[lints] workspace = true`) resolves to the workspace level, as
/// does the workspace manifest itself.
fn resolved_lint_level(manifest: Manifest<'_>, lint: &str) -> TestResult<Option<String>> {
    let document: Value = toml::from_str(manifest.contents)?;
    if let Some(level) = lint_level(&document, lint) {
        return Ok(Some(level));
    }

    let inherits_workspace = document
        .get("lints")
        .and_then(|lints| lints.get("workspace"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if inherits_workspace || manifest.path == "Cargo.toml" {
        let workspace: Value = toml::from_str(WORKSPACE_MANIFEST)?;
        let workspace_table = workspace
            .get("workspace")
            .ok_or("the workspace manifest must declare [workspace]")?;
        return Ok(lint_level(workspace_table, lint));
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
    let declared = disallowed_method_paths()?;
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
    for manifest in MANIFESTS_DENYING_DISALLOWED_METHODS {
        assert_eq!(
            resolved_lint_level(manifest, "disallowed_methods")?.as_deref(),
            Some("deny"),
            "{} must deny clippy::disallowed_methods",
            manifest.path
        );
    }
    Ok(())
}

/// Scenario: a package that denies the policy locally drops the companion
/// attribute lints, so a bare `#[allow]` becomes a viable escape hatch again.
///
/// Invariant: every locally denying manifest also denies `allow_attributes`
/// and `allow_attributes_without_reason`.
#[test]
fn a_local_deny_carries_the_companion_attribute_lints() -> TestResult {
    for manifest in MANIFESTS_WITH_A_LOCAL_DENY {
        let document: Value = toml::from_str(manifest.contents)?;
        for lint in COMPANION_ATTRIBUTE_LINTS {
            assert_eq!(
                lint_level(&document, lint).as_deref(),
                Some("deny"),
                "{} denies disallowed_methods locally, so it must deny \
                 clippy::{lint} too",
                manifest.path
            );
        }
    }
    Ok(())
}

/// Report whether the Makefile carries `statement` as a live directive.
///
/// The comparison is against a whole line with its trailing whitespace
/// removed, and commented-out lines are skipped. A substring search would be
/// satisfied by the same text inside a comment, certifying a default that Make
/// never applies.
fn makefile_declares(statement: &str) -> bool {
    MAKEFILE
        .lines()
        .filter(|line| !line.starts_with('\t'))
        .map(str::trim)
        .filter(|line| !line.trim_start().starts_with('#'))
        .any(|line| line == statement)
}

/// Report whether `statement` occurs in the recipe for the exact target.
fn makefile_target_recipe_contains(target: &str, statement: &str) -> bool {
    let mut is_target_recipe = false;

    for line in MAKEFILE.lines() {
        if line.starts_with('\t') {
            if is_target_recipe && line.trim() == statement {
                return true;
            }
        } else {
            is_target_recipe = line.trim() == target;
        }
    }

    false
}

/// Report whether an active Makefile directive exports `name` globally.
///
/// A bare `export` exports every Make variable to child processes, so it also
/// violates the test-only environment boundary.
fn makefile_globally_exports(makefile: &str, name: &str) -> bool {
    const MAKE_ASSIGNMENT_OPERATORS: &[&str] = &["=", ":=", "::=", ":::=", "?=", "+=", "!="];

    makefile
        .lines()
        .filter(|line| !line.starts_with('\t'))
        .map(str::trim)
        .filter(|line| !line.starts_with('#'))
        .any(|line| {
            let Some(arguments) = line.strip_prefix("export") else {
                return false;
            };
            if !arguments.is_empty() && !arguments.chars().next().is_some_and(char::is_whitespace) {
                return false;
            }

            let names: Vec<_> = arguments
                .split('#')
                .next()
                .unwrap_or_default()
                .split_whitespace()
                .collect();
            names.is_empty()
                || names.iter().any(|exported| {
                    *exported == name
                        || exported.strip_prefix(name).is_some_and(|suffix| {
                            MAKE_ASSIGNMENT_OPERATORS
                                .iter()
                                .any(|operator| suffix.starts_with(*operator))
                        })
                })
        })
}

/// Scenario: test-runner settings stop reaching the Rust test processes.
///
/// Invariant: the Makefile declares both overridable defaults and exports
/// them only inside the `test-rust` recipe, while unrelated recipes do not
/// inherit them. Each Make directive is matched as a whole, uncommented line:
/// a substring search would accept text inside a comment or global export.
#[test]
fn the_makefile_composes_the_embedded_postgres_environment() -> TestResult {
    for statement in [
        "PG_PASSWORD ?= wildside_embedded_test",
        "POSTGRESQL_RELEASES_URL ?= https://github.com/theseus-rs/postgresql-binaries",
        "shell_quote = '$(subst ','\"'\"',$(value $(1)))'",
        "test-rust: workspace-sync prepare-pg-worker",
    ] {
        assert!(
            makefile_declares(statement),
            "the Makefile must carry `{statement}` as a live directive, not \
             inside a comment"
        );
    }
    assert!(makefile_target_recipe_contains(
        "test-rust: workspace-sync prepare-pg-worker",
        "export PG_PASSWORD=$(call shell_quote,PG_PASSWORD) POSTGRESQL_RELEASES_URL=$(call shell_quote,POSTGRESQL_RELEASES_URL)",
    ));
    for name in ["PG_PASSWORD", "POSTGRESQL_RELEASES_URL"] {
        assert!(
            !makefile_globally_exports(MAKEFILE, name),
            "{name} must not be exported to unrelated Make recipes"
        );
    }
    Ok(())
}

/// Cargo may compose non-secret settings without replacing explicit caller
/// configuration or mutating the environment inside test processes.
#[rstest::rstest]
#[case::version("PG_VERSION_REQ", "=16.10.0")]
#[case::releases(
    "POSTGRESQL_RELEASES_URL",
    "https://github.com/theseus-rs/postgresql-binaries"
)]
fn cargo_supplies_non_forcing_postgres_defaults(
    #[case] name: &str,
    #[case] expected: &str,
) -> TestResult {
    let config: Value = toml::from_str(CARGO_CONFIG)?;
    let setting = config
        .get("env")
        .and_then(|env| env.get(name))
        .ok_or("Cargo must supply the non-secret PostgreSQL defaults")?;
    assert_eq!(setting.get("value").and_then(Value::as_str), Some(expected));
    assert_eq!(setting.get("force").and_then(Value::as_bool), Some(false));
    Ok(())
}

/// Cargo's process-wide environment must not expose the test password to
/// non-test commands such as the cluster initializer.
#[test]
fn cargo_does_not_supply_the_test_password_to_non_test_commands() -> TestResult {
    let config: Value = toml::from_str(CARGO_CONFIG)?;
    assert!(
        config
            .get("env")
            .and_then(|environment| environment.get("PG_PASSWORD"))
            .is_none(),
        "Cargo config must not apply the test password to every Cargo command"
    );
    Ok(())
}
