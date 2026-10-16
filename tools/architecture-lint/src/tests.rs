//! Unit tests for the architecture lint.

use camino::Utf8PathBuf;
use rstest::fixture;
use rstest::rstest;

use super::*;

#[derive(Clone, Copy)]
struct LintSingle;

impl LintSingle {
    fn lint(self, file: &str, contents: &str) -> Result<(), ArchitectureLintError> {
        lint_sources(&[LintSource {
            file: Utf8PathBuf::from(file),
            contents: contents.to_owned(),
        }])
    }
}

#[fixture]
fn lint_single() -> LintSingle {
    LintSingle
}

#[rstest]
#[case(
    "inbound/http/users.rs",
    "use crate::domain::UserId; fn handler() { let _ = UserId::new(\"x\"); }",
    true
)]
#[case(
    "inbound/http/users.rs",
    "use crate::outbound::persistence::DieselUserRepository; fn handler() { let _ = DieselUserRepository; }",
    false
)]
#[case(
    "inbound/http/users.rs",
    "use outbound::persistence::DieselUserRepository; fn handler() { let _ = DieselUserRepository; }",
    false
)]
#[case(
    "inbound/http/users.rs",
    "use backend::outbound::persistence::DieselUserRepository; fn handler() { let _ = DieselUserRepository; }",
    false
)]
#[case(
    "inbound/http/users.rs",
    "use diesel::prelude::*; fn handler() {}",
    false
)]
#[case(
    "domain/user.rs",
    "use crate::inbound::http; fn thing() { let _ = 1; }",
    false
)]
#[case(
    "outbound/persistence/user_repository.rs",
    "use crate::inbound::http; fn thing() { let _ = 1; }",
    false
)]
#[case(
    "outbound/persistence/user_repository.rs",
    "use inbound::http; fn thing() { let _ = 1; }",
    false
)]
#[case(
    "domain/user.rs",
    "use utoipa::ToSchema; #[derive(ToSchema)] struct Foo;",
    false
)]
#[case("domain/ports/metrics.rs", "use prometheus::Registry;", false)]
#[case("inbound/http/metrics.rs", "use prometheus::Registry;", false)]
#[case("domain/metrics.rs", "use actix_web_prom::*;", false)]
#[case("inbound/metrics.rs", "use actix_web_prom::*;", false)]
#[case("outbound/persistence/metrics.rs", "use prometheus::Registry;", false)]
#[case("outbound/metrics_backup.rs", "use prometheus::Registry;", false)]
#[case(
    "outbound/metrics/prometheus_idempotency.rs",
    "use prometheus::Registry;",
    true
)]
#[case(
    "outbound/queue/apalis_route_queue/tests/metrics.rs",
    "use prometheus::Registry;",
    true
)]
fn detects_boundary_violations(
    lint_single: LintSingle,
    #[case] file: &str,
    #[case] contents: &str,
    #[case] ok: bool,
) {
    let result = lint_single.lint(file, contents);
    assert_eq!(result.is_ok(), ok, "result: {result:?}");
}
