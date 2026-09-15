//! The scan's verdict, over the whole space of attribute spellings.
//!
//! The cases in `environment_policy_source_scan.rs` pin the shapes that were
//! measured against Clippy. They cannot cover the space those shapes are drawn
//! from: scope, keyword, lint name, `cfg_attr` nesting, raw spelling and a
//! reason string multiply out faster than anyone will write them down, and the
//! defects found on this contract so far have all been combinations nobody
//! thought to write.
//!
//! So the rule is stated once, in `Shape::is_offence`, and the property is
//! that the scan agrees with it on every generated attribute. The rule is
//! three lines:
//!
//! - an unprotected lint is never an offence, however it is spelled;
//! - `allow` of a protected lint always is, inner or outer;
//! - `expect` of one is an offence only when it is inner, because a
//!   crate-scoped expectation is fulfilled by the first prohibited call and
//!   silences the rest forever, while the item-scoped form still reports them.
//!
//! `deny` is generated too, and is never an offence. It is the control: a
//! judgement that keyed on "an attribute mentioning a protected lint" rather
//! than on the keyword would pass every other case here and fail this one.
//!
//! # Mutation proof
//!
//! Recorded 2026-09-14. Each was applied to the judgement, run through the
//! build, and reverted. The two fail from opposite directions, which is what
//! a one-sided property would not have caught:
//!
//! - dropping the scope guard from `expect`, so an item-scoped one is judged
//!   like a crate-scoped one, fails this property, shrunk to an outer
//!   `#[expect(clippy::disallowed_methods)]` with no reason and no nesting.
//!   That is precisely the form the guide prescribes, so the mutation turns
//!   the sanctioned attribute into a finding;
//! - replacing the exact path comparison with a prefix test fails it too,
//!   shrunk to `#![allow(clippy::alloc_instead_of_core)]`, a lint whose name
//!   begins with `clippy::all` and which nothing protects.
//!
//! Proptest wrote both shrunk cases to a regressions file, which is not
//! committed: they came from deliberate mutations rather than from a defect,
//! and both are already pinned as named cases in
//! `environment_policy_source_scan.rs`.

#[expect(
    dead_code,
    reason = "this binary uses the attribute judgement only; the source \
              reading is exercised by environment_policy_source_scan.rs"
)]
mod environment_policy_scan;

use environment_policy_scan::{PROTECTED_LINTS, suppressed_lints};
use proptest::prelude::*;

/// Lint names the policy does not protect, chosen to be adversarial.
///
/// The first begins with `clippy::all` and the second with `clippy::style`,
/// so a substring test would report both. The third begins with `warnings`.
/// Comparing paths rather than prefixes is what keeps them out of the
/// findings, and a contributor who cannot tell a real finding from a false
/// one stops reading them.
const UNPROTECTED_LINTS: [&str; 4] = [
    "clippy::alloc_instead_of_core",
    "clippy::styled_toggle",
    "warnings_extra",
    "dead_code",
];

/// Reason strings carrying the punctuation that defeats a text scan.
const REASONS: [&str; 3] = [
    "see the note (below)",
    "one, two, three",
    "closes with a ] bracket",
];

/// Which of the three lint attributes the generated shape carries.
#[derive(Debug, Clone, Copy)]
enum Keyword {
    /// Silent forever; always an offence over a protected lint.
    Allow,
    /// Sanctioned when item-scoped, an evasion when crate-scoped.
    Expect,
    /// Never a suppression. The control.
    Deny,
}

impl Keyword {
    /// Return the keyword as it is written in an attribute.
    fn spelling(self) -> &'static str {
        match self {
            Self::Allow => "allow",
            Self::Expect => "expect",
            Self::Deny => "deny",
        }
    }
}

/// One generated attribute, and the file it is rendered into.
#[derive(Debug, Clone)]
struct Shape {
    /// Whether the attribute is written `#![...]` rather than `#[...]`.
    inner: bool,
    keyword: Keyword,
    /// The lint the attribute names.
    lint: &'static str,
    /// Whether that lint is one the policy protects.
    protected: bool,
    /// How many `cfg_attr` layers wrap the attribute.
    depth: usize,
    /// Whether the lint's last path segment is spelled `r#...`.
    raw: bool,
    /// The reason string, when the attribute carries one.
    reason: Option<&'static str>,
}

impl Shape {
    /// Return the lint path as the attribute spells it.
    fn lint_spelling(&self) -> String {
        if !self.raw {
            return self.lint.to_owned();
        }
        self.lint.rsplit_once("::").map_or_else(
            || format!("r#{}", self.lint),
            |(head, tail)| format!("{head}::r#{tail}"),
        )
    }

    /// Render the shape as a Rust source file.
    ///
    /// An outer attribute needs an item to sit on, so one is appended; an
    /// inner attribute stands alone at the top of the file.
    fn render(&self) -> String {
        let reason = self
            .reason
            .map_or_else(String::new, |reason| format!(", reason = \"{reason}\""));
        let mut meta = format!(
            "{}({}{reason})",
            self.keyword.spelling(),
            self.lint_spelling()
        );
        for _ in 0..self.depth {
            meta = format!("cfg_attr(all(), {meta})");
        }
        let bang = if self.inner { "!" } else { "" };
        let item = if self.inner { "" } else { "\nfn probe() {}" };
        format!("#{bang}[{meta}]{item}\n")
    }

    /// Whether the scan must report this shape, stated once.
    fn is_offence(&self) -> bool {
        if !self.protected {
            return false;
        }
        match self.keyword {
            Keyword::Allow => true,
            Keyword::Expect => self.inner,
            Keyword::Deny => false,
        }
    }
}

/// Return the lint at `index` across both lists, and whether it is protected.
fn lint_at(index: usize) -> (&'static str, bool) {
    PROTECTED_LINTS.get(index).map_or_else(
        || {
            let offset = index.saturating_sub(PROTECTED_LINTS.len());
            (UNPROTECTED_LINTS[offset], false)
        },
        |lint| (*lint, true),
    )
}

/// Generate one attribute shape.
///
/// `cfg_attr` depth stops at two: the nesting is followed recursively and the
/// outermost scope is carried through, so a third layer exercises nothing a
/// second does not.
fn shapes() -> impl Strategy<Value = Shape> {
    let lints = 0..PROTECTED_LINTS.len() + UNPROTECTED_LINTS.len();
    let keywords = prop_oneof![
        Just(Keyword::Allow),
        Just(Keyword::Expect),
        Just(Keyword::Deny),
    ];
    let reasons = proptest::option::of(prop_oneof![
        Just(REASONS[0]),
        Just(REASONS[1]),
        Just(REASONS[2]),
    ]);
    (
        any::<bool>(),
        keywords,
        lints,
        0_usize..=2,
        any::<bool>(),
        reasons,
    )
        .prop_map(|(inner, keyword, index, depth, raw, reason)| {
            let (lint, protected) = lint_at(index);
            Shape {
                inner,
                keyword,
                lint,
                protected,
                depth,
                raw,
                reason,
            }
        })
}

proptest! {
    /// Scenario: any attribute the generator can spell.
    ///
    /// Invariant: the scan reports exactly one finding when the rule says the
    /// shape is an offence, and none otherwise. Both directions matter. A
    /// missed finding is a bypass; a spurious one is a false positive, and a
    /// contract that produces those gets switched off, which is the same
    /// outcome by a slower route.
    #[test]
    fn the_verdict_follows_the_rule(shape in shapes()) {
        let source = shape.render();
        let found = suppressed_lints(&source)
            .map_err(|error| TestCaseError::fail(format!("{source:?}: {error}")))?;
        let expected = usize::from(shape.is_offence());

        prop_assert_eq!(
            found.len(),
            expected,
            "{:?} must yield {} finding(s), got {:?}",
            source,
            expected,
            found
        );
    }
}
