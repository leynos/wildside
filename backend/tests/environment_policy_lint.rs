//! Proof that the environment-access policy's Clippy configuration actually
//! fires (issue #464).
//!
//! `environment_policy_contract.rs` asserts what the repository declares. This
//! target asserts what Clippy *does* with those declarations: it materializes
//! a probe source into a temporary directory and runs `clippy-driver` over it
//! with `CLIPPY_CONF_DIR` pointed at the workspace root, then reads the JSON
//! diagnostics. One process per probe, no nested Cargo, no fixture package.
//!
//! Each reason string is asserted inside its own diagnostic rather than
//! against the whole output: `var`, `var_os`, `vars`, and `vars_os` share a
//! reason, so a whole-output search would survive a mutation that rewrote one
//! of them.
//!
//! # Mutation proof
//!
//! Mutation-tested on 2026-09-07, one mutation at a time, each reverted
//! afterwards:
//!
//! - rewriting the `std::env::var_os` reason in `clippy.toml` to
//!   `inject a reader` failed `the_lint_reports_every_prohibited_call` with
//!   `std::env::var_os must be reported with its configured reason`;
//! - deleting the `std::env::vars` entry failed the same test with
//!   `expected exactly 6 disallowed_methods diagnostics, got 5`.

use std::error::Error as StdError;
use std::path::{Path, PathBuf};
use std::process::Command;

use cap_std::ambient_authority;
use cap_std::fs::Dir;
use serde_json::Value as JsonValue;

#[path = "support/test_env.rs"]
mod test_env;

type TestResult<T = ()> = Result<T, Box<dyn StdError>>;

const PROHIBITED_CALLS_PROBE: &str =
    include_str!("fixtures/environment_policy/prohibited_calls.rs.txt");
const ALLOW_ATTRIBUTE_PROBE: &str =
    include_str!("fixtures/environment_policy/allow_attribute.rs.txt");
const EXPECT_ATTRIBUTE_PROBE: &str =
    include_str!("fixtures/environment_policy/expect_attribute.rs.txt");
const CONTROL_PROBE: &str = include_str!("fixtures/environment_policy/control.rs.txt");

/// The prohibited APIs paired with the reason `clippy.toml` configures.
const PROHIBITED_APIS_AND_REASONS: [(&str, &str); 6] = [
    ("std::env::var", "inject an environment reader"),
    ("std::env::var_os", "inject an environment reader"),
    ("std::env::vars", "inject an environment reader"),
    ("std::env::vars_os", "inject an environment reader"),
    (
        "std::env::set_var",
        "inject the value, or configure the subprocess; in tests use a stub environment",
    ),
    (
        "std::env::remove_var",
        "inject the value, or configure the subprocess; in tests use a stub environment",
    ),
];

/// One Clippy diagnostic, reduced to the fields the policy cares about.
struct Diagnostic {
    code: Option<String>,
    message: String,
    notes: Vec<String>,
}

impl Diagnostic {
    fn has_code(&self, code: &str) -> bool {
        self.code.as_deref() == Some(code)
    }
}

/// Return the workspace root.
fn workspace_root() -> TestResult<PathBuf> {
    Ok(Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("the backend manifest directory must have a parent")?
        .to_path_buf())
}

/// Forward one variable from the test process to the child, when it is set.
///
/// The child's environment is built explicitly from a cleared base, so nothing
/// the harness happens to carry — coverage instrumentation in particular —
/// reaches the probe compilation.
fn forward(command: &mut Command, name: &str) {
    if let Some(value) = test_env::process_env(name) {
        command.env(name, value);
    }
}

/// Compile `probe` with `clippy-driver` and return its diagnostics.
fn lint_probe(probe: &str) -> TestResult<Vec<Diagnostic>> {
    let sandbox = tempfile::tempdir()?;
    let sandbox_dir = Dir::open_ambient_dir(sandbox.path(), ambient_authority())?;
    sandbox_dir.write("probe.rs", probe)?;
    sandbox_dir.create_dir("out")?;
    let root = workspace_root()?;

    let mut command = Command::new("clippy-driver");
    command.env_clear();
    command.env(
        "PATH",
        test_env::process_env("PATH").ok_or("PATH must be set")?,
    );
    for name in ["HOME", "RUSTUP_HOME", "RUSTUP_TOOLCHAIN", "CARGO_HOME"] {
        forward(&mut command, name);
    }
    // The repository's own configuration must decide the outcome.
    command.env("CLIPPY_CONF_DIR", &root);
    command.current_dir(&root);
    command.args([
        "--edition",
        "2024",
        "--crate-type",
        "lib",
        "--emit=metadata",
        // `allow_attributes` is allow-by-default, so the probe that reaches for
        // `#[allow]` needs it switched on to be judged at all.
        "-W",
        "clippy::allow_attributes",
        "--error-format=json",
    ]);
    command.arg(sandbox.path().join("probe.rs"));
    command.arg("--out-dir").arg(sandbox.path().join("out"));

    let output = command.output()?;
    let stderr = String::from_utf8(output.stderr)?;
    Ok(stderr.lines().filter_map(parse_diagnostic).collect())
}

/// Parse one JSON line into a [`Diagnostic`], skipping non-diagnostic lines
/// and the trailing "N warnings emitted" summary.
fn parse_diagnostic(line: &str) -> Option<Diagnostic> {
    let value: JsonValue = serde_json::from_str(line).ok()?;
    if value.get("$message_type")?.as_str()? != "diagnostic" {
        return None;
    }
    let message = value.get("message")?.as_str()?.to_owned();
    if message.ends_with("warnings emitted") || message.ends_with("warning emitted") {
        return None;
    }
    let code = value
        .get("code")
        .and_then(|code| code.get("code"))
        .and_then(JsonValue::as_str)
        .map(str::to_owned);
    let notes = value
        .get("children")
        .and_then(JsonValue::as_array)
        .map(|children| {
            children
                .iter()
                .filter(|child| child.get("level").and_then(JsonValue::as_str) == Some("note"))
                .filter_map(|child| child.get("message").and_then(JsonValue::as_str))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    Some(Diagnostic {
        code,
        message,
        notes,
    })
}

/// Scenario: a source calls each of the six prohibited process-environment
/// APIs once.
///
/// Invariant: Clippy reports exactly six `disallowed_methods` diagnostics, and
/// each names its API and carries that API's configured reason string. A
/// seventh diagnostic would mean a sanctioned `#[expect]` had stopped being
/// honoured; a sixth reason in the wrong diagnostic would mean an entry had
/// been rewritten.
#[test]
fn the_lint_reports_every_prohibited_call() -> TestResult {
    let diagnostics = lint_probe(PROHIBITED_CALLS_PROBE)?;
    let reported: Vec<&Diagnostic> = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.has_code("clippy::disallowed_methods"))
        .collect();

    assert_eq!(
        reported.len(),
        PROHIBITED_APIS_AND_REASONS.len(),
        "expected exactly {} disallowed_methods diagnostics, got {}: {:?}",
        PROHIBITED_APIS_AND_REASONS.len(),
        reported.len(),
        reported
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>()
    );

    for (api, reason) in PROHIBITED_APIS_AND_REASONS {
        let diagnostic = reported
            .iter()
            .find(|diagnostic| diagnostic.message.contains(api))
            .ok_or_else(|| format!("{api} must be reported by clippy::disallowed_methods"))?;
        assert!(
            diagnostic.notes.iter().any(|note| note == reason),
            "{api} must be reported with its configured reason `{reason}`; \
             notes were {:?}",
            diagnostic.notes
        );
    }
    Ok(())
}

/// Scenario: a contributor reaches for `#[allow]` instead of `#[expect]` to
/// silence a prohibited call.
///
/// Invariant: the allow does suppress `disallowed_methods` — which is why the
/// policy forbids it — and `clippy::allow_attributes` objects to the attribute
/// itself, so the escape hatch is closed rather than merely discouraged.
#[test]
fn an_allow_attribute_is_rejected() -> TestResult {
    let diagnostics = lint_probe(ALLOW_ATTRIBUTE_PROBE)?;

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.has_code("clippy::allow_attributes")),
        "an #[allow] over a prohibited call must be reported by \
         clippy::allow_attributes"
    );
    assert!(
        !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.has_code("clippy::disallowed_methods")),
        "the #[allow] suppresses disallowed_methods, which is exactly why the \
         policy requires #[expect] instead"
    );
    Ok(())
}

/// Scenario: a genuine composition root carries a reasoned `#[expect]`, and a
/// control source uses an injected reader instead of the process.
///
/// Invariant: both lint clean. A diagnostic on the control would mean the
/// harness reports noise and the other probes' results could not be trusted; a
/// diagnostic on the `#[expect]` would mean the sanctioned escape hatch does
/// not work.
#[test]
fn a_reasoned_expect_and_an_injected_reader_lint_clean() -> TestResult {
    for (name, probe) in [
        ("the reasoned #[expect] probe", EXPECT_ATTRIBUTE_PROBE),
        ("the injected-reader control", CONTROL_PROBE),
    ] {
        let diagnostics = lint_probe(probe)?;
        assert!(
            diagnostics.is_empty(),
            "{name} must lint clean; got {:?}",
            diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.as_str())
                .collect::<Vec<_>>()
        );
    }
    Ok(())
}
