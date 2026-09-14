//! What the scan must catch, and what it must not report.
//!
//! The file beside this one runs the scan over the workspace. These are the
//! probes: one file per shape, under
//! `backend/tests/fixtures/environment_policy_scan/`, each carrying the
//! `.rs.txt` suffix so the scan does not read its own counterexamples as
//! sources. Every shape here was measured against this repository's own
//! Clippy before it was added; the measurements and the mutation record live
//! in `environment_policy_source_scan.rs`.
//!
//! Roughly half the cases are negative. A contract that reports a false
//! positive gets switched off, so each rule carries the case that keeps it
//! narrow beside the case that proves it reaches.

#[expect(
    dead_code,
    reason = "these cases drive the judgement only; the workspace reading is \
              exercised by environment_policy_source_scan.rs"
)]
mod environment_policy_scan;

use environment_policy_scan::{PROTECTED_LINTS, TestResult, suppressed_lints, unreadable_includes};
use rstest::rstest;

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
/// A protected `allow` on an item nested two modules deep.
const NESTED_ITEM_ALLOW: &str =
    include_str!("fixtures/environment_policy_scan/nested_item_allow.rs.txt");
/// A crate-scoped `expect` inside a module declared within a function body.
const FUNCTION_LOCAL_EXPECT: &str =
    include_str!("fixtures/environment_policy_scan/function_local_expect.rs.txt");
/// A macro arm forwarding an attribute, invoked with a protected `allow`.
const MACRO_FORWARDED_ALLOW: &str =
    include_str!("fixtures/environment_policy_scan/macro_forwarded_allow.rs.txt");
/// The same arm, invoked with lints the policy does not protect.
const MACRO_FORWARDED_INNOCENT: &str =
    include_str!("fixtures/environment_policy_scan/macro_forwarded_innocent.rs.txt");
/// An `include!` of a target the scan does not read.
const INCLUDE_UNREADABLE: &str =
    include_str!("fixtures/environment_policy_scan/include_unreadable.rs.txt");
/// An `include!` naming a `.rs` file, the form this workspace uses.
const INCLUDE_RUST_PATH: &str =
    include_str!("fixtures/environment_policy_scan/include_rust_path.rs.txt");
/// An `include!` whose target is computed rather than named.
const INCLUDE_COMPUTED: &str =
    include_str!("fixtures/environment_policy_scan/include_computed.rs.txt");
/// `include_str!` and `include_bytes!`, which embed data rather than source.
const INCLUDE_STR_IS_DATA: &str =
    include_str!("fixtures/environment_policy_scan/include_str_is_data.rs.txt");
/// A macro arm forwarding attributes and lint names as metavariables.
const MACRO_METAVARIABLE: &str =
    include_str!("fixtures/environment_policy_scan/macro_metavariable.rs.txt");
/// The same shape, one macro definition deeper.
const MACRO_TWO_DEEP: &str = include_str!("fixtures/environment_policy_scan/macro_two_deep.rs.txt");

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

/// Scenario: each protected lint, suppressed at crate scope both ways.
///
/// Invariant: every one is reported, under `allow` and under `expect` alike.
/// The set is finite and written down, so it is covered entry by entry rather
/// than sampled: a lint added to `PROTECTED_LINTS` but not reachable by the
/// judgement, or one quietly dropped from it, is what this catches. The probe
/// is built from the constant rather than held as a fixture precisely so the
/// set drives the cases.
#[rstest]
fn every_protected_lint_is_reported_at_crate_scope(
    #[values(0, 1, 2, 3, 4, 5, 6)] index: usize,
    #[values("allow", "expect")] keyword: &str,
) -> TestResult {
    let lint = PROTECTED_LINTS[index];
    let probe = format!("#![{keyword}({lint}, reason = \"x\")]\n");

    let found = suppressed_lints(&probe)?;
    assert_eq!(found.len(), 1, "{keyword} of {lint} was missed: {found:?}");
    assert_eq!(found[0].0, lint, "{lint} was reported under another name");
    Ok(())
}

/// Scenario: the suppression sits on an item two modules deep.
///
/// Invariant: it is reported. The walk is a visitor rather than a scan of the
/// file's leading attributes, so depth in the module tree is not a hiding
/// place; a reader that only inspected top-level items would miss this.
#[test]
fn a_suppression_on_a_nested_item_is_an_offence() -> TestResult {
    assert_eq!(suppressed_lints(NESTED_ITEM_ALLOW)?.len(), 1);
    Ok(())
}

/// Scenario: a crate-scoped `expect` inside a module declared in a function.
///
/// Invariant: it is reported. A function-local module has its own crate-level
/// attribute position, so `#![expect(...)]` there suppresses every prohibited
/// call in it while sitting somewhere no one reads.
#[test]
fn a_suppression_in_a_function_local_module_is_an_offence() -> TestResult {
    assert_eq!(suppressed_lints(FUNCTION_LOCAL_EXPECT)?.len(), 1);
    Ok(())
}

/// Scenario: a macro arm forwarding an attribute and a lint as metavariables.
///
/// Invariant: none of it is an offence. `#[$attribute]` and `#[allow($lint)]`
/// are not suppressions until the macro is expanded, and `#[allow(dead_code,
/// reason = "generated")]` is what a code-generating macro legitimately
/// emits. Reporting any of them would make every such macro a finding, and a
/// contract that reports false positives gets switched off. This is the case
/// that keeps the token walk honest, and it is why a shape that fails to
/// parse inside a macro body is passed over rather than raised.
#[test]
fn macro_metavariables_are_not_suppressions() -> TestResult {
    assert!(
        suppressed_lints(MACRO_METAVARIABLE)?.is_empty(),
        "a macro forwarding attributes was reported as a suppression"
    );
    Ok(())
}

/// Scenario: a macro arm forwards an attribute the invocation supplies.
///
/// Invariant: it is reported. Measured on 2026-09-14, `#[$attribute]` over a
/// prohibited call, invoked as `forward!(allow(clippy::disallowed_methods))`,
/// took that call from one diagnostic to none. Neither half is a suppression
/// on its own: the transcriber's attribute is a metavariable, and the
/// invocation's argument is not an attribute at all, so both passed every
/// earlier form of this scan. The argument is the half that names the lint,
/// so that is the half judged.
#[test]
fn a_forwarded_suppression_is_an_offence() -> TestResult {
    assert_eq!(suppressed_lints(MACRO_FORWARDED_ALLOW)?.len(), 1);
    Ok(())
}

/// Scenario: the same arm, forwarding lints the policy does not protect.
///
/// Invariant: neither invocation is an offence. Forwarding an attribute is
/// ordinary macro practice, so the judgement has to turn on the lint named
/// rather than on the forwarding. Without this the rule would report every
/// code-generating macro in the workspace and be switched off within a week.
#[test]
fn forwarding_an_unprotected_lint_is_not_an_offence() -> TestResult {
    assert!(
        suppressed_lints(MACRO_FORWARDED_INNOCENT)?.is_empty(),
        "forwarding dead_code or unused_variables was reported"
    );
    Ok(())
}

/// Scenario: an `include!` of a target the scan does not read.
///
/// Invariant: it is reported. rustc parses an `include!` target as Rust
/// whatever its extension, so an `#![allow(clippy::disallowed_methods)]` at
/// the top of an included `.rs.txt` silences the calls around it. Measured on
/// 2026-09-14, that took a prohibited call from one diagnostic to none while
/// this scan, which reads `.rs` files, never saw the attribute. This
/// repository's own probe fixtures carry that suffix, so the route is
/// reachable with the files already in the tree.
#[test]
fn an_include_of_an_unreadable_target_is_an_offence() -> TestResult {
    assert_eq!(unreadable_includes(INCLUDE_UNREADABLE)?.len(), 1);
    Ok(())
}

/// Scenario: an `include!` whose target is computed at compile time.
///
/// Invariant: it is reported. The finding is about reachability, not content:
/// the scan cannot know what `concat!(env!("OUT_DIR"), ...)` names, and a
/// route it cannot see is exactly what the last four bypasses had in common.
/// Naming the file is the remedy.
#[test]
fn an_include_of_a_computed_target_is_an_offence() -> TestResult {
    assert_eq!(unreadable_includes(INCLUDE_COMPUTED)?.len(), 1);
    Ok(())
}

/// Scenario: the `include!` form this workspace actually uses.
///
/// Invariant: it is not an offence. Every `include!` under `backend/` names
/// `support/entrypoint.rs` literally, and that file is one the scan already
/// reads, so nothing is hidden behind it.
#[test]
fn an_include_naming_a_rust_file_is_not_an_offence() -> TestResult {
    assert!(
        unreadable_includes(INCLUDE_RUST_PATH)?.is_empty(),
        "the workspace's own include! form was reported"
    );
    Ok(())
}

/// Scenario: `include_str!` and `include_bytes!` over the same target.
///
/// Invariant: neither is an offence. They embed a file as data, so an
/// attribute inside one is text and suppresses nothing. This distinction is
/// not incidental: the scan's own probe fixtures are loaded that way, and a
/// rule that did not draw it would report the contract's own test file.
#[test]
fn embedding_a_file_as_data_is_not_an_offence() -> TestResult {
    assert!(
        unreadable_includes(INCLUDE_STR_IS_DATA)?.is_empty(),
        "include_str! or include_bytes! was read as source inclusion"
    );
    Ok(())
}
