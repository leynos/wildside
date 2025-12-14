//! Behaviour tests for the architecture guardrails.

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use architecture_lint::{ArchitectureLintError, LintSource, Violation};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use tempfile::TempDir;

#[derive(Debug, Default)]
struct LintWorld {
    sources: Vec<LintSource>,
    result: Option<Result<(), ArchitectureLintError>>,
}

#[fixture]
fn world() -> Mutex<LintWorld> {
    Mutex::new(LintWorld::default())
}

fn add_source(world: &Mutex<LintWorld>, file: &str, contents: &str) {
    let mut world = world.lock().expect("world lock");
    world.sources.push(LintSource {
        file: PathBuf::from(file),
        contents: contents.to_owned(),
    })
}

#[given("an inbound module that imports the outbound layer")]
fn inbound_imports_outbound(world: &Mutex<LintWorld>) {
    add_source(
        world,
        "inbound/http/users.rs",
        "use backend::outbound::persistence::user_repository; fn handler() { let _ = user_repository::DieselUserRepository; }",
    );
}

#[given("an inbound module that imports Diesel directly")]
fn inbound_imports_diesel(world: &Mutex<LintWorld>) {
    add_source(
        world,
        "inbound/http/users.rs",
        "use diesel::prelude::*; fn handler() {}",
    );
}

#[given("a domain module that imports Actix Web")]
fn domain_imports_actix(world: &Mutex<LintWorld>) {
    add_source(
        world,
        "domain/user.rs",
        "use actix_web::HttpResponse; fn handler() { let _ = HttpResponse::Ok(); }",
    );
}

#[given("an outbound module that imports the inbound layer")]
fn outbound_imports_inbound(world: &Mutex<LintWorld>) {
    add_source(
        world,
        "outbound/persistence/bad_cross_boundary.rs",
        "use crate::inbound::http; fn handler() { let _ = 1; }",
    );
}

#[given("valid domain, inbound, and outbound modules")]
fn valid_modules(world: &Mutex<LintWorld>) {
    add_valid_modules(world);
}

#[given("valid modules mixed with multiple boundary violations")]
fn valid_modules_with_multiple_violations(world: &Mutex<LintWorld>) {
    add_valid_modules(world);
    add_source(
        world,
        "inbound/http/bad_cross_boundary.rs",
        "use backend::outbound::persistence::user_repository; fn handler() { let _ = user_repository::DieselUserRepository; }",
    );
    add_source(
        world,
        "domain/bad.rs",
        "use actix_web::HttpResponse; fn handler() { let _ = HttpResponse::Ok(); }",
    );
}

fn add_valid_modules(world: &Mutex<LintWorld>) {
    add_source(
        world,
        "domain/user.rs",
        "pub struct UserId(String); impl UserId { pub fn new(v: &str) -> Self { Self(v.to_owned()) } }",
    );
    add_source(
        world,
        "inbound/http/users.rs",
        "use crate::domain::user::UserId; fn handler() { let _id = UserId::new(\"ok\"); }",
    );
    add_source(
        world,
        "outbound/persistence/user_repository.rs",
        "use crate::domain::user::UserId; pub struct Repo; impl Repo { pub fn save(&self, _id: UserId) {} }",
    );
}

#[when("the architecture lint runs")]
fn run_architecture_lint(world: &Mutex<LintWorld>) {
    let sources = {
        let world = world.lock().expect("world lock");
        world.sources.clone()
    };

    let temp_dir = TempDir::new().expect("tempdir");
    let backend_dir = temp_dir.path().join("backend");
    let src_dir = backend_dir.join("src");
    for source in &sources {
        let path = src_dir.join(&source.file);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent directories");
        }
        fs::write(&path, &source.contents).expect("write source file");
    }

    let result = architecture_lint::lint_backend_sources(&backend_dir);
    let mut world = world.lock().expect("world lock");
    world.result = Some(result);
}

#[then("the lint succeeds")]
fn lint_succeeds(world: &Mutex<LintWorld>) {
    let world = world.lock().expect("world lock");
    let outcome = world.result.as_ref().expect("lint must have run");
    assert!(outcome.is_ok(), "expected success, got: {outcome:?}");
}

fn assert_violation_in_file_contains(
    world: &Mutex<LintWorld>,
    expected_file: &str,
    expected_substring: &str,
) {
    let expected_file = PathBuf::from(expected_file);
    let violations = violations(world);
    assert!(
        violations.iter().any(|violation| {
            violation.file == expected_file && violation.message.contains(expected_substring)
        }),
        "expected violation in '{expected_file:?}' containing '{expected_substring}', got: {violations:?}"
    );
}

fn violations(world: &Mutex<LintWorld>) -> Vec<Violation> {
    let world = world.lock().expect("world lock");
    let outcome = world.result.as_ref().expect("lint must have run");
    extract_violations(outcome).expect("expected violations")
}

#[then("the lint fails due to outbound access from inbound")]
fn lint_fails_due_to_outbound_access(world: &Mutex<LintWorld>) {
    assert_violation_in_file_contains(world, "inbound/http/users.rs", "crate::outbound");
}

#[then("the lint fails due to inbound access from outbound")]
fn lint_fails_due_to_inbound_access(world: &Mutex<LintWorld>) {
    assert_violation_in_file_contains(
        world,
        "outbound/persistence/bad_cross_boundary.rs",
        "crate::inbound",
    );
}

#[then("the lint fails due to infrastructure crate usage")]
fn lint_fails_due_to_infrastructure_crate(world: &Mutex<LintWorld>) {
    assert_violation_in_file_contains(world, "inbound/http/users.rs", "external crate `diesel`");
}

#[then("the lint fails due to framework crate usage in the domain")]
fn lint_fails_due_to_framework_crate(world: &Mutex<LintWorld>) {
    assert_violation_in_file_contains(world, "domain/user.rs", "external crate `actix_web`");
}

#[then("the lint fails")]
fn lint_fails(world: &Mutex<LintWorld>) {
    let world = world.lock().expect("world lock");
    let outcome = world.result.as_ref().expect("lint must have run");
    assert!(outcome.is_err(), "expected failure, got: {outcome:?}");
}

#[then("all boundary violations are reported")]
fn all_boundary_violations_are_reported(world: &Mutex<LintWorld>) {
    let violations = violations(world);
    assert!(
        violations.len() >= 2,
        "expected at least 2 violations, got: {violations:?}"
    );
    assert_violation_in_file_contains(
        world,
        "inbound/http/bad_cross_boundary.rs",
        "crate::outbound",
    );
    assert_violation_in_file_contains(world, "domain/bad.rs", "external crate `actix_web`");
}

fn extract_violations(outcome: &Result<(), ArchitectureLintError>) -> Option<Vec<Violation>> {
    match outcome {
        Ok(()) => None,
        Err(ArchitectureLintError::Violations(violations)) => Some(violations.clone()),
        Err(other) => panic!("expected violations error, got: {other:?}"),
    }
}

#[scenario(path = "tests/features/architecture_guardrails.feature")]
fn architecture_guardrails(world: Mutex<LintWorld>) {
    let _ = world;
}
