//! Command-boundary tests for the Makefile's test environment.
//!
//! The sibling tests read the Makefile as text. These tests instead run `make`
//! as a child process against a probe target, so they prove what Make actually
//! does: the defaults it supplies, the shell word `shell_quote` produces for a
//! caller override, and the absence of the test password from unrelated
//! recipes. A text assertion cannot show that a value survives shell parsing.
//!
//! The recipe itself is exercised too. `test-rust` runs under `.ONESHELL`, so
//! its `export` line affects the later compile commands, and only running the
//! target shows that. A fake `cargo` records the environment it received,
//! which keeps the check to the export rather than to a real compile.

use std::os::unix::fs::PermissionsExt;
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
    // `MAKEFLAGS`, `MFLAGS`, and `MAKELEVEL` carry the parent invocation's
    // `-n`, `-s`, or job-control flags, which would change or suppress the
    // probe recipe rather than measure the Makefile under test.
    command
        .env_remove("PG_PASSWORD")
        .env_remove("POSTGRESQL_RELEASES_URL")
        .env_remove("MAKEFLAGS")
        .env_remove("MFLAGS")
        .env_remove("MAKELEVEL")
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

/// The `cargo` double: it records the two settings and reports success.
const FAKE_CARGO: &str = r#"#!/bin/sh
printf '%s\n' "${PG_PASSWORD-unset}" > "$CARGO_ENV_LOG"
printf '%s\n' "${POSTGRESQL_RELEASES_URL-unset}" >> "$CARGO_ENV_LOG"
exit 0
"#;

/// One recorded `make` run: its output and what its `cargo` double saw.
struct TargetRun {
    output: Output,
    recorded: String,
}

/// Run the real `test-rust` recipe with a `cargo` double.
///
/// A command-line `PATH` assignment overrides the Makefile's own
/// `export PATH :=`, so the double is the `cargo` the recipe invokes and no
/// compilation happens. `-o` marks the prerequisites as up to date: workspace
/// sync and the embedded-worker build are not what this contract measures.
fn run_test_rust(assignments: &[(&str, &str)]) -> TargetRun {
    let sandbox = tempfile::tempdir().expect("tempdir");
    let bin = sandbox.path().join("bin");
    std::fs::create_dir(&bin).expect("create the fake bin directory");
    let fake_cargo = bin.join("cargo");
    std::fs::write(&fake_cargo, FAKE_CARGO).expect("write the fake cargo tool");
    let mut permissions = std::fs::metadata(&fake_cargo)
        .expect("stat the fake cargo tool")
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&fake_cargo, permissions).expect("make the fake cargo executable");
    let log_path = sandbox.path().join("cargo-env.log");

    let mut command = Command::new("make");
    command
        .env_remove("PG_PASSWORD")
        .env_remove("POSTGRESQL_RELEASES_URL")
        .env_remove("MAKEFLAGS")
        .env_remove("MFLAGS")
        .env_remove("MAKELEVEL")
        .env("CARGO_ENV_LOG", &log_path)
        .current_dir(repository_root())
        .arg("--no-print-directory")
        .arg("-f")
        .arg("Makefile")
        .arg("-o")
        .arg("workspace-sync")
        .arg("-o")
        .arg("prepare-pg-worker")
        .arg("test-rust")
        .arg(format!("PATH={}:/usr/bin:/bin", bin.display()));
    for (name, value) in assignments {
        command.arg(format!("{name}={value}"));
    }
    let output = command.output().expect("make runs as a child process");
    let recorded = std::fs::read_to_string(&log_path).unwrap_or_default();
    TargetRun { output, recorded }
}

#[test]
fn test_rust_exports_the_documented_defaults_to_its_tools() {
    let run = run_test_rust(&[]);

    assert!(
        run.output.status.success(),
        "the test-rust recipe must succeed, stderr: {}",
        String::from_utf8_lossy(&run.output.stderr)
    );
    assert_eq!(
        run.recorded, "wildside_embedded_test\nhttps://github.com/theseus-rs/postgresql-binaries\n",
        "the compile commands must receive the documented defaults"
    );
}

#[test]
fn test_rust_exports_caller_overrides_verbatim() {
    // Spaces, a dollar sign, double quotes, a single quote, and shell
    // metacharacters prove the export survives both word splitting and every
    // expansion the `.ONESHELL` script performs.
    let password = r#"p@ss w$rd"x'y"#;
    let releases_url = r#"https://example.test/a b?x=1&y="2""#;
    let run = run_test_rust(&[
        ("PG_PASSWORD", password),
        ("POSTGRESQL_RELEASES_URL", releases_url),
    ]);

    assert!(
        run.output.status.success(),
        "the test-rust recipe must succeed, stderr: {}",
        String::from_utf8_lossy(&run.output.stderr)
    );
    assert_eq!(
        run.recorded,
        format!("{password}\n{releases_url}\n"),
        "a caller override must reach the compile commands unchanged"
    );
}

#[test]
fn make_does_not_export_the_test_password_to_unrelated_targets() {
    let output = run_probe("@env", "probe-env", &[]);
    let environment = probe_stdout(output);

    for setting in ["PG_PASSWORD=", "POSTGRESQL_RELEASES_URL="] {
        assert!(
            !environment.lines().any(|line| line.starts_with(setting)),
            "an unrelated recipe must not inherit the test {setting} setting"
        );
    }
}
