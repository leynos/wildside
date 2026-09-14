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

mod environment_policy_scan;

use rstest::rstest;

use environment_policy_scan::{
    SOURCE_ROOTS, TestResult, rust_sources, suppressed_lints, workspace_root,
};

/// An `expect` on the composition-root item: the form the guide prescribes.
const SANCTIONED_EXPECT: &str =
    include_str!("fixtures/environment_policy_scan/sanctioned_expect.rs.txt");
/// An `allow` reached through a conditional attribute.
const CFG_ATTR_ALLOW: &str = include_str!("fixtures/environment_policy_scan/cfg_attr_allow.rs.txt");
/// An `allow` naming the group Clippy places the policy lint in.
const GROUP_ALLOW: &str = include_str!("fixtures/environment_policy_scan/group_allow.rs.txt");
/// A space before the parenthesis, and a parenthesis inside the reason.
const AWKWARD_SPACING: &str =
    include_str!("fixtures/environment_policy_scan/awkward_spacing.rs.txt");
/// Attribute-shaped text in a doc comment and in a string literal.
const ATTRIBUTE_SHAPED_TEXT: &str =
    include_str!("fixtures/environment_policy_scan/attribute_shaped_text.rs.txt");
/// A lint whose name merely begins with a protected one.
const LONGER_LINT_NAME: &str =
    include_str!("fixtures/environment_policy_scan/longer_lint_name.rs.txt");
/// A crate-scoped `expect`: the sanctioned form's evasive twin.
const CRATE_SCOPED_EXPECT: &str =
    include_str!("fixtures/environment_policy_scan/crate_scoped_expect.rs.txt");
/// A crate-scoped `expect` reached through a conditional attribute.
const CFG_ATTR_EXPECT: &str =
    include_str!("fixtures/environment_policy_scan/cfg_attr_expect.rs.txt");
/// The attribute itself spelled as a raw identifier.
const RAW_ATTRIBUTE: &str = include_str!("fixtures/environment_policy_scan/raw_attribute.rs.txt");
/// A raw identifier inside the lint path.
const RAW_LINT_PATH: &str = include_str!("fixtures/environment_policy_scan/raw_lint_path.rs.txt");
/// `warnings` spelled as a raw identifier.
const RAW_WARNINGS: &str = include_str!("fixtures/environment_policy_scan/raw_warnings.rs.txt");
/// A `macro_rules!` arm expanding to a module with an inner `allow`.
const MACRO_BODY: &str = include_str!("fixtures/environment_policy_scan/macro_body.rs.txt");
/// The same shape, one macro definition deeper.
const MACRO_TWO_DEEP: &str = include_str!("fixtures/environment_policy_scan/macro_two_deep.rs.txt");

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

/// Scenario: an `expect` at a sanctioned composition root.
///
/// Invariant: the scan does not reject it. Rejecting `expect` would push
/// contributors towards `allow`, the attribute that never warns.
#[test]
fn a_sanctioned_expect_is_not_an_offence() -> TestResult {
    assert!(suppressed_lints(SANCTIONED_EXPECT)?.is_empty());
    Ok(())
}

/// Scenario: the suppression is nested inside a `cfg_attr`.
///
/// Invariant: it is reported. Clippy honours the nested `allow`,
/// `clippy::allow_attributes` does not report it, and a scan looking for a
/// line beginning `#![allow(` would miss it entirely. The condition is not
/// evaluated: a suppression that applies under some configuration is still a
/// suppression.
#[test]
fn a_suppression_nested_in_cfg_attr_is_an_offence() -> TestResult {
    assert_eq!(suppressed_lints(CFG_ATTR_ALLOW)?.len(), 1);
    Ok(())
}

/// Scenario: the suppression names the lint's group rather than the lint.
///
/// Invariant: it is reported. Clippy places `disallowed_methods` in `style`,
/// so `#![allow(clippy::style)]` switches the policy off without ever naming
/// it.
#[test]
fn a_suppression_of_the_lints_group_is_an_offence() -> TestResult {
    assert_eq!(suppressed_lints(GROUP_ALLOW)?.len(), 1);
    Ok(())
}

/// Scenario: awkward spacing, and a parenthesis inside the reason string.
///
/// Invariant: both are handled. A scan matching a fixed opener would miss the
/// spaced form, and one counting raw parentheses would end the attribute early
/// at the parenthesis inside the string.
#[test]
fn spacing_and_a_parenthesis_in_the_reason_do_not_hide_a_suppression() -> TestResult {
    assert_eq!(suppressed_lints(AWKWARD_SPACING)?.len(), 1);
    Ok(())
}

/// Scenario: attribute-shaped text inside a string literal or a doc comment.
///
/// Invariant: neither is an offence. This is the false positive a text scan
/// cannot avoid, and a contract that reports one gets disabled. This file's
/// own documentation quotes the attribute repeatedly.
#[test]
fn attribute_shaped_text_is_not_an_attribute() -> TestResult {
    assert!(suppressed_lints(ATTRIBUTE_SHAPED_TEXT)?.is_empty());
    Ok(())
}

/// Scenario: a lint whose name merely contains a protected one.
///
/// Invariant: `clippy::alloc_instead_of_core` is not reported. Its name begins
/// with `clippy::all`, so a substring test would reject it and leave a
/// contributor unable to tell a real finding from a false one. Comparing paths
/// is what prevents that. Note also that it is a `restriction` lint, and
/// `clippy::restriction` is protected: naming a lint is not naming its group.
#[test]
fn a_longer_lint_name_containing_a_protected_one_is_not_an_offence() -> TestResult {
    assert!(suppressed_lints(LONGER_LINT_NAME)?.is_empty());
    Ok(())
}

/// Scenario: a crate-scoped `#![expect]` naming a protected lint.
///
/// Invariant: it is reported, although an item-scoped `expect` is not.
/// Measured, a crate root carrying `#![expect(clippy::disallowed_methods)]`
/// reported nothing for two separate prohibited calls and raised no
/// `unfulfilled_lint_expectations`, because one call fulfils the expectation
/// for the whole crate. The same file with an item-scoped `#[expect]` still
/// reported the second call. An inner `expect` is therefore a silent,
/// permanent suppression wearing the sanctioned form's clothes.
#[test]
fn a_crate_scoped_expect_is_an_offence() -> TestResult {
    assert_eq!(suppressed_lints(CRATE_SCOPED_EXPECT)?.len(), 1);
    Ok(())
}

/// Scenario: a crate-scoped `expect` reached through a `cfg_attr`.
///
/// Invariant: it is reported too. The scope of the outermost attribute is
/// carried through the nesting, so the conditional form is judged as the
/// crate-scoped expectation it becomes.
#[test]
fn a_crate_scoped_expect_nested_in_cfg_attr_is_an_offence() -> TestResult {
    assert_eq!(suppressed_lints(CFG_ATTR_EXPECT)?.len(), 1);
    Ok(())
}

/// Scenario: the attribute or the lint is spelled as a raw identifier.
///
/// Invariant: both are reported. `r#allow` and `clippy::r#style` name exactly
/// the same attribute and lint as their plain spellings, and both were
/// measured to suppress the policy lint, but `syn` renders an identifier with
/// its `r#` prefix intact. Normalizing before comparison is what catches them.
#[rstest]
#[case::the_attribute(RAW_ATTRIBUTE)]
#[case::the_lint_path(RAW_LINT_PATH)]
#[case::a_bare_lint_name(RAW_WARNINGS)]
fn a_raw_identifier_does_not_hide_a_suppression(#[case] probe: &str) -> TestResult {
    assert_eq!(suppressed_lints(probe)?.len(), 1, "missed {probe}");
    Ok(())
}

/// Scenario: the suppression sits two macro definitions deep.
///
/// Invariant: it is reported. A `macro_rules!` arm that defines a second
/// macro carrying the attribute nests the suppression inside two token
/// streams; the walk recurses through every group, so depth does not hide it.
/// Measured, the two-deep shape takes the prohibited call from one diagnostic
/// to none, exactly as the one-deep shape does.
#[test]
fn a_suppression_two_macros_deep_is_an_offence() -> TestResult {
    assert_eq!(suppressed_lints(MACRO_TWO_DEEP)?.len(), 1);
    Ok(())
}

/// Scenario: the suppression sits inside a macro body.
///
/// Invariant: it is reported. `syn` leaves a macro body as opaque tokens, so
/// the visitor never sees an inner attribute in an arm expanding to a module.
/// Measured, that shape takes the prohibited call from one diagnostic to none
/// while Clippy's own guard stays silent, which is why the token stream is
/// walked as well as the syntax tree.
#[test]
fn a_suppression_inside_a_macro_body_is_an_offence() -> TestResult {
    assert_eq!(suppressed_lints(MACRO_BODY)?.len(), 1);
    Ok(())
}
