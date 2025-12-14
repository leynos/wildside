//! Behaviour tests for the architecture guardrails.

use std::path::PathBuf;
use std::sync::Mutex;

use architecture_lint::{lint_sources, ArchitectureLintError, LintSource, Violation};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

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
        "use crate::outbound::persistence::user_repository; fn handler() { let _ = user_repository::DieselUserRepository; }",
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

#[given("valid domain, inbound, and outbound modules")]
fn valid_modules(world: &Mutex<LintWorld>) {
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
    let result = lint_sources(&sources);
    let mut world = world.lock().expect("world lock");
    world.result = Some(result);
}

#[then("the lint succeeds")]
fn lint_succeeds(world: &Mutex<LintWorld>) {
    let world = world.lock().expect("world lock");
    let outcome = world.result.as_ref().expect("lint must have run");
    assert!(outcome.is_ok(), "expected success, got: {outcome:?}");
}

#[then("the lint fails due to outbound access from inbound")]
fn lint_fails_due_to_outbound_access(world: &Mutex<LintWorld>) {
    let world = world.lock().expect("world lock");
    let outcome = world.result.as_ref().expect("lint must have run");
    let violations = extract_violations(outcome).expect("expected violations");
    assert!(
        violations
            .iter()
            .any(|violation| violation.message.contains("crate::outbound")),
        "expected crate::outbound violation, got: {violations:?}"
    );
}

#[then("the lint fails due to infrastructure crate usage")]
fn lint_fails_due_to_infrastructure_crate(world: &Mutex<LintWorld>) {
    let world = world.lock().expect("world lock");
    let outcome = world.result.as_ref().expect("lint must have run");
    let violations = extract_violations(outcome).expect("expected violations");
    assert!(
        violations
            .iter()
            .any(|violation| violation.message.contains("external crate `diesel`")),
        "expected diesel violation, got: {violations:?}"
    );
}

#[then("the lint fails due to framework crate usage in the domain")]
fn lint_fails_due_to_framework_crate(world: &Mutex<LintWorld>) {
    let world = world.lock().expect("world lock");
    let outcome = world.result.as_ref().expect("lint must have run");
    let violations = extract_violations(outcome).expect("expected violations");
    assert!(
        violations
            .iter()
            .any(|violation| violation.message.contains("external crate `actix_web`")),
        "expected actix_web violation, got: {violations:?}"
    );
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
