//! Source scan closing the attribute route around the environment policy.
//!
//! The sibling contracts check the configuration and prove the lint fires.
//! Neither sees a source file that switches the lint off for itself. A
//! crate-level inner attribute does exactly that:
//!
//! ```ignore
//! #![allow(clippy::disallowed_methods, reason = "...")]
//! ```
//!
//! `clippy::allow_attributes`, which `backend` and `architecture-lint` deny,
//! does not fire on inner attributes, so a whole crate can stop enforcing the
//! policy with every other gate still green.
//!
//! Naming the lint is only one of the ways in. All five forms below were
//! measured against this repository's own Clippy on 2026-09-08, by running
//! `clippy-driver` with `CLIPPY_CONF_DIR` at the workspace root over a probe
//! calling `std::env::var`. The unsuppressed probe reported one
//! `disallowed_methods` diagnostic; each of these reported none:
//!
//! - `#![allow(clippy::disallowed_methods)]`, naming the lint;
//! - `#![allow(clippy::style)]`, naming the group Clippy places it in;
//! - `#![allow(clippy::all)]`, naming the wider group;
//! - `#![allow(warnings)]`, taking down everything;
//! - `#![cfg_attr(all(), allow(clippy::disallowed_methods))]`, reached through
//!   a conditional attribute.
//!
//! An `#[expect(clippy::disallowed_methods, reason = "...")]` over the same
//! call still reported its diagnostic, which is why `expect` is the sanctioned
//! form at a composition root and why this scan leaves it alone: `expect`
//! warns once its site no longer needs it, `allow` is silent forever.
//!
//! The guard that enforces that choice is defeatable in its own right, also
//! measured: `#![allow(clippy::restriction)]` takes `clippy::allow_attributes`
//! from one diagnostic to none while leaving `disallowed_methods` reporting.
//! It disarms the guard rather than the policy, which is why the group and
//! both guard lints are protected too.
//!
//! The sources are parsed rather than searched. A text scan cannot tell an
//! attribute from attribute-shaped text in a string literal, cannot follow
//! `cfg_attr`, and ends an attribute early at a parenthesis inside a `reason`.
//! Parsing alone is not sufficient either: raw identifiers are normalized
//! before comparison, `expect` is judged by its scope rather than ignored, and
//! macro token streams are walked because `syn` leaves a macro body opaque.
//!
//! # Mutation proof
//!
//! Recorded 2026-09-08. Each was applied alone to a real workspace source,
//! run through the build, and reverted:
//!
//! - `#![allow(clippy::disallowed_methods)]` at the top of
//!   `backend/src/lib.rs` fails `no_source_file_allows_a_policy_lint`;
//! - `#![allow(clippy::style)]` there fails it, naming the group rather than
//!   the lint;
//! - `#![cfg_attr(all(), allow(clippy::disallowed_methods))]` there fails it;
//! - `#![allow(clippy::all)]` split over several lines fails it;
//! - `#[allow(warnings)]` on an item in `backend/src/main.rs` fails it;
//! - `#[allow(clippy::restriction)]` in a crate root fails it, because that
//!   group holds the two guard lints that keep `allow` from being used where
//!   the policy requires `expect`;
//! - `#![expect(clippy::disallowed_methods)]` in a crate root fails it. An
//!   item-scoped `expect` is the sanctioned form and is not reported, but a
//!   crate-scoped one is a silent, permanent suppression: measured, it took
//!   two separate prohibited calls to zero diagnostics with no
//!   `unfulfilled_lint_expectations`, because one call fulfils the whole
//!   crate's expectation, where the item-scoped form still reported the
//!   second call;
//! - `#![allow(clippy::r#style)]` fails it. A raw identifier names the same
//!   lint and is honoured by Clippy, but `syn` renders the `r#` prefix, so the
//!   comparison normalizes it away;
//! - a `macro_rules!` arm expanding to `mod inner { #![allow(...)] }` fails
//!   it. `syn` leaves a macro body opaque, so the token stream is walked as
//!   well as the syntax tree;
//! - `#[allow(clippy::alloc_instead_of_core)]` must and does keep passing. Its
//!   name begins with `clippy::all`, so a substring test would reject it, and
//!   it is a `restriction` lint, so a group-aware test would too. Comparing
//!   paths is what keeps both from being false reports.
//!
//! Two further routes, measured 2026-09-14 against this repository's own
//! Clippy with `clippy-driver --crate-type lib` and `CLIPPY_CONF_DIR` at the
//! workspace root. The unsuppressed probe reported one `disallowed_methods`
//! diagnostic; each of these reported none:
//!
//! - a `macro_rules!` arm writing `#[$attribute]` over a prohibited call,
//!   invoked as `forward!(allow(clippy::disallowed_methods))`. Neither half
//!   is a suppression alone: the transcriber's attribute is a metavariable
//!   that no parser can resolve, and the invocation's argument is not an
//!   attribute at all. The argument is the half that names the lint, so that
//!   is the half judged, and only when the lint is protected;
//! - `include!("something.rs.txt")`, because rustc parses an `include!`
//!   target as Rust whatever its extension. An `#![allow(...)]` at the top of
//!   the included file silences the calls around it while this scan, which
//!   reads `.rs` files, never sees it. This contract's own probe fixtures
//!   carry that suffix, so the route was reachable with files already in the
//!   tree. An `include!` is therefore a finding unless its target is a string
//!   literal ending in `.rs`; every `include!` in this workspace names
//!   `support/entrypoint.rs` literally and is unaffected.
//!
//! Both were confirmed open against the scan as it stood and closed after:
//! dropped into `backend/` as real sources, the scan passed before the change
//! and named both files after it.
//!
//! Re-run 2026-09-14, after the scan moved to its own module:
//! `#![allow(clippy::style)]` in `backend/src/lib.rs` still fails
//! `no_source_file_allows_a_policy_lint`, and a probe fixture copied from
//! `group_allow.rs.txt` to `leaked_probe.rs` fails it too, which is what the
//! `.rs.txt` suffix exists to prevent. The two mutations run against the
//! judgement itself are recorded in `environment_policy_scan_properties.rs`.
//!
//! The two routes above carry four more, each run through the build. Reach:
//! removing the forwarded-attribute arm fails
//! `a_forwarded_suppression_is_an_offence`; treating every `include!` target
//! as readable fails both `an_include_of_*` cases. Narrowness, which matters
//! as much, because a contract that reports false positives gets switched
//! off: adding `dead_code` to the protected set fails
//! `forwarding_an_unprotected_lint_is_not_an_offence`, and treating
//! `include_str!` as source inclusion fails both
//! `embedding_a_file_as_data_is_not_an_offence` and the workspace scan, since
//! this repository embeds files that way throughout.

mod environment_policy_scan;

use environment_policy_scan::{
    SOURCE_ROOTS, TestResult, rust_sources, suppressed_lints, unreadable_includes, workspace_root,
};

/// Scenario: a source file switches the policy lint off for itself.
///
/// Invariant: no Rust source in the workspace allows a protected lint. An
/// inner attribute is the case that matters, because `clippy::allow_attributes`
/// cannot see one, so nothing else in the repository would notice.
#[test]
fn no_source_file_allows_a_policy_lint() -> TestResult {
    let root = workspace_root()?;
    let mut offences = Vec::new();

    for source_root in SOURCE_ROOTS {
        let sources = rust_sources(&root.join(source_root), source_root)?;
        assert!(
            !sources.is_empty(),
            "{source_root} should contain Rust sources to scan"
        );
        for (path, contents) in sources {
            let display = path.display();
            let offending = suppressed_lints(&contents)
                .map_err(|error| format!("{display} should parse as Rust: {error}"))?;
            for (lint, attribute) in offending {
                offences.push(format!("{display} allows {lint} via {attribute}"));
            }
        }
    }

    assert!(
        offences.is_empty(),
        "no source may allow a protected lint; use an item-scoped \
         #[expect(..., reason = \"...\")] at a composition root instead:\n{}",
        offences.join("\n")
    );
    Ok(())
}

/// Scenario: the scan is pointed at sources that no longer exist.
///
/// Invariant: it reads real files and fails loudly rather than passing by
/// finding nothing, which is how a source scan usually rots.
#[test]
fn the_scan_reads_the_workspace_sources() -> TestResult {
    let root = workspace_root()?;
    let mut sources = Vec::new();
    for source_root in SOURCE_ROOTS {
        sources.extend(rust_sources(&root.join(source_root), source_root)?);
    }

    assert!(
        sources.len() > 300,
        "the scan should cover the workspace's sources, found {}",
        sources.len()
    );
    for expected in [
        "backend/src/lib.rs",
        "crates/example-data/src/lib.rs",
        "tools/architecture-lint/src/main.rs",
    ] {
        assert!(
            sources
                .iter()
                .any(|(path, _)| path.to_string_lossy().replace('\\', "/") == expected),
            "the scan should reach {expected}"
        );
    }
    Ok(())
}

/// Scenario: the workspace's own sources, read for unreadable inclusions.
///
/// Invariant: no source includes a target this scan cannot read. The sibling
/// test proves no source allows a protected lint; this one proves none of
/// them reaches past the scan to a file where such an allow could sit
/// unexamined.
#[test]
fn no_source_file_includes_an_unreadable_target() -> TestResult {
    let root = workspace_root()?;
    let mut offences = Vec::new();

    for source_root in SOURCE_ROOTS {
        for (path, contents) in rust_sources(&root.join(source_root), source_root)? {
            let display = path.display();
            let found = unreadable_includes(&contents, &path)
                .map_err(|error| format!("{display} should parse as Rust: {error}"))?;
            for inclusion in found {
                offences.push(format!("{display} includes {inclusion}"));
            }
        }
    }

    assert!(
        offences.is_empty(),
        "an include! target must be a string literal naming a .rs file inside \
         the scanned roots, or the scan cannot see what it brought in:\n{}",
        offences.join("\n")
    );
    Ok(())
}
