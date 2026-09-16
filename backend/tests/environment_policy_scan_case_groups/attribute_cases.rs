//! The attribute suppressions the scan must catch, and the ones it must not.
//!
//! Every shape here was measured against this repository's own Clippy before
//! it was added; the measurements and the mutation record live in
//! `environment_policy_source_scan.rs`. The probes are one file per shape
//! under `backend/tests/fixtures/environment_policy_scan/`, each carrying the
//! `.rs.txt` suffix so the scan does not read its own counterexamples as
//! sources.
//!
//! Roughly half the cases are negative. A contract that reports a false
//! positive gets switched off, so each rule carries the case that keeps it
//! narrow beside the case that proves it reaches.

use rstest::rstest;

use crate::environment_policy_scan::{PROTECTED_LINTS, TestResult, suppressed_lints};

/// An `expect` on the composition-root item: the form the guide prescribes.
const SANCTIONED_EXPECT: &str =
    include_str!("../fixtures/environment_policy_scan/sanctioned_expect.rs.txt");
/// An `allow` reached through a conditional attribute.
const CFG_ATTR_ALLOW: &str =
    include_str!("../fixtures/environment_policy_scan/cfg_attr_allow.rs.txt");
/// An `allow` naming the group Clippy places the policy lint in.
const GROUP_ALLOW: &str = include_str!("../fixtures/environment_policy_scan/group_allow.rs.txt");
/// A space before the parenthesis, and a parenthesis inside the reason.
const AWKWARD_SPACING: &str =
    include_str!("../fixtures/environment_policy_scan/awkward_spacing.rs.txt");
/// Attribute-shaped text in a doc comment and in a string literal.
const ATTRIBUTE_SHAPED_TEXT: &str =
    include_str!("../fixtures/environment_policy_scan/attribute_shaped_text.rs.txt");
/// A lint whose name merely begins with a protected one.
const LONGER_LINT_NAME: &str =
    include_str!("../fixtures/environment_policy_scan/longer_lint_name.rs.txt");
/// A crate-scoped `expect`: the sanctioned form's evasive twin.
const CRATE_SCOPED_EXPECT: &str =
    include_str!("../fixtures/environment_policy_scan/crate_scoped_expect.rs.txt");
/// A crate-scoped `expect` reached through a conditional attribute.
const CFG_ATTR_EXPECT: &str =
    include_str!("../fixtures/environment_policy_scan/cfg_attr_expect.rs.txt");
/// The attribute itself spelled as a raw identifier.
const RAW_ATTRIBUTE: &str =
    include_str!("../fixtures/environment_policy_scan/raw_attribute.rs.txt");
/// A raw identifier inside the lint path.
const RAW_LINT_PATH: &str =
    include_str!("../fixtures/environment_policy_scan/raw_lint_path.rs.txt");
/// `warnings` spelled as a raw identifier.
const RAW_WARNINGS: &str = include_str!("../fixtures/environment_policy_scan/raw_warnings.rs.txt");
/// A `macro_rules!` arm expanding to a module with an inner `allow`.
const MACRO_BODY: &str = include_str!("../fixtures/environment_policy_scan/macro_body.rs.txt");
/// A protected `allow` on an item nested two modules deep.
const NESTED_ITEM_ALLOW: &str =
    include_str!("../fixtures/environment_policy_scan/nested_item_allow.rs.txt");
/// A crate-scoped `expect` inside a module declared within a function body.
const FUNCTION_LOCAL_EXPECT: &str =
    include_str!("../fixtures/environment_policy_scan/function_local_expect.rs.txt");
/// A macro arm forwarding an attribute, invoked with a protected `allow`.
const MACRO_FORWARDED_ALLOW: &str =
    include_str!("../fixtures/environment_policy_scan/macro_forwarded_allow.rs.txt");
/// The same arm, invoked with lints the policy does not protect.
const MACRO_FORWARDED_INNOCENT: &str =
    include_str!("../fixtures/environment_policy_scan/macro_forwarded_innocent.rs.txt");
/// A macro arm forwarding attributes and lint names as metavariables.
const MACRO_METAVARIABLE: &str =
    include_str!("../fixtures/environment_policy_scan/macro_metavariable.rs.txt");
/// The same shape, one macro definition deeper.
const MACRO_TWO_DEEP: &str =
    include_str!("../fixtures/environment_policy_scan/macro_two_deep.rs.txt");

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
/// Iterating the constant is part of that. An indexed case list has to be
/// edited alongside it, and the forgotten edit leaves the newest entry, the
/// one nobody has judged yet, as the only untested member of the set.
#[test]
fn every_protected_lint_is_reported_at_crate_scope() -> TestResult {
    for lint in PROTECTED_LINTS {
        for keyword in ["allow", "expect"] {
            let probe = format!("#![{keyword}({lint}, reason = \"x\")]\n");

            let found = suppressed_lints(&probe)?;
            assert_eq!(found.len(), 1, "{keyword} of {lint} was missed: {found:?}");
            assert_eq!(found[0].0, lint, "{lint} was reported under another name");
        }
    }
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
