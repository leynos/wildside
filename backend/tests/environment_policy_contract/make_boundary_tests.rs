//! Command-boundary tests for the Makefile's test environment.
//!
//! The sibling tests read the Makefile as text. These tests instead run `make`
//! as a child process against a probe target, so they prove what Make actually
//! does: the defaults it supplies, the shell word `shell_quote` produces for a
//! caller override, and the absence of the test password from unrelated
//! recipes. A text assertion cannot show that a value survives shell parsing.

use std::path::PathBuf;
use std::process::{Command, Output};

/// Repository root, one level above the `backend` package directory.
fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the backend package lives inside the repository")
        .to_path_buf()
}

/// Run one probe target from the repository root.
///
/// `probe` is the recipe body, `target` its name, and `assignments` are
/// command-line variable overrides. Arguments are passed as argv, so no shell
/// quoting is involved on the way in.
fn run_probe(probe: &str, target: &str, assignments: &[(&str, &str)]) -> Output {
    let mut command = Command::new("make");
    // `make test` exports these into the test process, so a child `make` would
    // inherit them and the probes would measure the parent, not the Makefile.
    command
        .env_remove("PG_PASSWORD")
        .env_remove("POSTGRESQL_RELEASES_URL")
        .current_dir(repository_root())
        .arg("--no-print-directory")
        .arg("-f")
        .arg("Makefile")
        .arg("--eval")
        .arg(format!("{target}: ; {probe}"))
        .arg(target);
    for (name, value) in assignments {
        command.arg(format!("{name}={value}"));
    }
    command.output().expect("make runs as a child process")
}

/// Return a probe's standard output, failing with its standard error.
fn probe_stdout(output: Output) -> String {
    assert!(
        output.status.success(),
        "the probe target must succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("probe output is UTF-8")
}

#[test]
fn make_supplies_the_documented_test_defaults() {
    let output = run_probe(
        r#"@printf "%s|%s\n" "$(PG_PASSWORD)" "$(POSTGRESQL_RELEASES_URL)""#,
        "probe-defaults",
        &[],
    );

    assert_eq!(
        probe_stdout(output).trim_end(),
        "wildside_embedded_test|https://github.com/theseus-rs/postgresql-binaries",
        "an unset caller must receive the documented defaults"
    );
}

#[test]
fn make_preserves_a_caller_override_with_shell_special_characters() {
    // Spaces, a dollar sign, double quotes, and a single quote together prove
    // the quoting survives both word splitting and the `subst` that escapes
    // single quotes.
    let caller_value = r#"p@ss w$rd"x'y"#;
    let output = run_probe(
        r#"@printf "%s\n" $(call shell_quote,PG_PASSWORD)"#,
        "probe-quote",
        &[("PG_PASSWORD", caller_value)],
    );

    assert_eq!(
        probe_stdout(output).trim_end(),
        caller_value,
        "the quoted export must reach the shell as the caller's exact value"
    );
}

#[test]
fn make_does_not_export_the_test_password_to_unrelated_targets() {
    let output = run_probe("@env", "probe-env", &[]);
    let environment = probe_stdout(output);

    assert!(
        !environment
            .lines()
            .any(|line| line.starts_with("PG_PASSWORD=")),
        "an unrelated recipe must not inherit the test password"
    );
}
