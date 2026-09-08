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

use std::collections::VecDeque;
use std::error::Error as StdError;
use std::path::{Path as StdPath, PathBuf};

use cap_std::ambient_authority;
use cap_std::fs::Dir;
use proc_macro2::{Delimiter, TokenStream, TokenTree};
use syn::ext::IdentExt;
use syn::punctuated::Punctuated;
use syn::visit::Visit;
use syn::{AttrStyle, Attribute, Macro, Meta, MetaList, Path, Token};

type TestResult<T = ()> = Result<T, Box<dyn StdError>>;

/// Lints whose suppression disarms the environment-access policy, or the
/// guard that keeps its escape hatch honest.
///
/// The first four protect the policy itself. Naming the lint alone is not
/// enough: Clippy places `disallowed_methods` in the `style` group, so
/// `clippy::style` and the wider `clippy::all` each switch it off, and
/// `warnings` takes down everything.
///
/// The last three protect the guard. `backend` and `architecture-lint` deny
/// `clippy::allow_attributes` so that a prohibited call cannot be silenced
/// with an `allow` instead of an `expect`; allowing either guard lint, or the
/// `restriction` group that holds them, switches that deny off. Measured:
/// `#![allow(clippy::restriction)]` takes `clippy::allow_attributes` from one
/// diagnostic to none while leaving `disallowed_methods` reporting, so it
/// disarms the guard alone rather than the policy.
///
/// Extend this list if either lint's group changes.
const PROTECTED_LINTS: [&str; 7] = [
    "clippy::disallowed_methods",
    "clippy::style",
    "clippy::all",
    "warnings",
    "clippy::allow_attributes",
    "clippy::allow_attributes_without_reason",
    "clippy::restriction",
];

/// Directories holding the Rust sources the policy governs.
///
/// `third_party` is deliberately absent: it holds vendored code that is not a
/// workspace member and that the policy does not claim to govern.
const SOURCE_ROOTS: [&str; 3] = ["backend", "crates", "tools"];

/// Return the workspace root, from this crate's manifest directory.
fn workspace_root() -> TestResult<PathBuf> {
    Ok(StdPath::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("the backend manifest directory must have a parent")?
        .to_path_buf())
}

/// Collect every `.rs` file under one root, depth first.
///
/// Paths are reported relative to the workspace root, because that is what a
/// contributor acting on a failure needs to open.
fn rust_sources(root: &StdPath, relative: &str) -> TestResult<Vec<(PathBuf, String)>> {
    let directory = Dir::open_ambient_dir(root, ambient_authority())?;
    let mut pending = VecDeque::from([(directory, PathBuf::from(relative))]);
    let mut sources = Vec::new();

    while let Some((current, prefix)) = pending.pop_front() {
        for candidate in current.entries()? {
            let entry = candidate?;
            let name = entry.file_name();
            let path = prefix.join(&name);
            if entry.file_type()?.is_dir() {
                pending.push_back((current.open_dir(&name)?, path));
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let contents = current.read_to_string(&name)?;
                sources.push((path, contents));
            }
        }
    }
    Ok(sources)
}

/// Collect every attribute in a parsed file, and every macro token stream.
///
/// A visitor is used rather than a hand-rolled walk so attributes on nested
/// items, on function-local items, and on expressions are all reached. Macro
/// bodies are opaque to the visitor, so their token streams are collected for
/// a separate pass: a `macro_rules!` arm expanding to a module with an inner
/// `allow` would otherwise hide a suppression from both this scan and Clippy's
/// own guard.
#[derive(Default)]
struct AttributeCollector {
    attributes: Vec<Attribute>,
    macro_tokens: Vec<TokenStream>,
}

impl<'ast> Visit<'ast> for AttributeCollector {
    fn visit_attribute(&mut self, attribute: &'ast Attribute) {
        self.attributes.push(attribute.clone());
    }

    fn visit_macro(&mut self, mac: &'ast Macro) {
        self.macro_tokens.push(mac.tokens.clone());
        syn::visit::visit_macro(self, mac);
    }
}

/// Render a lint path as it is written in an attribute, with raw identifiers
/// normalized.
///
/// The path in `#[allow(clippy::all)]` renders as `clippy::all`, and the path
/// in `#[allow(warnings)]` as `warnings`. `r#` prefixes are stripped, because
/// `#![allow(clippy::r#style)]` and `#![r#allow(...)]` name exactly the same
/// lint and attribute as their plain spellings, and Clippy honours both.
fn render_path(path: &Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.ident.unraw().to_string())
        .collect::<Vec<_>>()
        .join("::")
}

/// Return the lint names an `allow` meta-list suppresses.
///
/// Key-value arguments such as `reason = "..."` are not lint names and are
/// skipped, so `allow(clippy::all, reason = "x")` yields `["clippy::all"]`.
fn allowed_lints(list: &MetaList) -> Vec<String> {
    let Ok(nested) = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated) else {
        return Vec::new();
    };
    nested
        .iter()
        .filter_map(|meta| match meta {
            Meta::Path(path) => Some(render_path(path)),
            Meta::List(_) | Meta::NameValue(_) => None,
        })
        .collect()
}

/// Whether an attribute is written `#![...]` rather than `#[...]`.
///
/// The distinction decides how an `expect` is judged, so it is carried through
/// `cfg_attr` nesting and through macro token streams rather than inferred at
/// the point of comparison.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Scope {
    /// `#![...]`, applying to the enclosing crate or module.
    Inner,
    /// `#[...]`, applying to the item it precedes.
    Outer,
}

/// Return the lint names nested inside a `cfg_attr`.
///
/// The leading element is the condition and is skipped; a nested `cfg_attr` is
/// followed in turn, so `cfg_attr(all(), allow(clippy::style))` yields
/// `["clippy::style"]`. The scope of the outermost attribute is carried in, so
/// `#![cfg_attr(all(), expect(clippy::all))]` is judged as the crate-scoped
/// expectation it becomes.
fn suppressed_by_cfg_attr(list: &MetaList, scope: Scope) -> Vec<String> {
    let Ok(nested) = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated) else {
        return Vec::new();
    };
    nested
        .iter()
        .skip(1)
        .flat_map(|meta| lints_from_meta(meta, scope))
        .collect()
}

/// Return the lint names one attribute meta suppresses.
///
/// A `cfg_attr` is followed whatever its condition: a suppression that applies
/// under some configuration is still a suppression, and deciding which
/// configurations are reachable is not this contract's job.
///
/// An `expect` is judged by its scope. An outer, item-scoped `expect` is the
/// sanctioned form and yields nothing: measured, it suppresses only its own
/// site, leaving a second prohibited call in the same file still reported. An
/// inner `expect` does not behave that way. Measured, a crate root carrying
/// `#![expect(clippy::disallowed_methods)]` reported nothing for two separate
/// calls and raised no `unfulfilled_lint_expectations`, because one call
/// fulfils the expectation for the whole crate. That is a silent, permanent
/// suppression wearing the sanctioned form's clothes, so it is an offence.
fn lints_from_meta(meta: &Meta, scope: Scope) -> Vec<String> {
    let Meta::List(list) = meta else {
        return Vec::new();
    };
    match render_path(&list.path).as_str() {
        "allow" => allowed_lints(list),
        "expect" if scope == Scope::Inner => allowed_lints(list),
        "cfg_attr" => suppressed_by_cfg_attr(list, scope),
        _ => Vec::new(),
    }
}

/// Return the lint names one attribute suppresses.
fn suppressed_by(attribute: &Attribute) -> Vec<String> {
    let scope = match attribute.style {
        AttrStyle::Inner(_) => Scope::Inner,
        AttrStyle::Outer => Scope::Outer,
    };
    lints_from_meta(&attribute.meta, scope)
}

/// Return every protected lint suppressed by an attribute inside a macro's
/// token stream.
///
/// `syn` treats a macro body as opaque tokens, so an arm expanding to
/// `mod inner { #![allow(clippy::disallowed_methods)] ... }` is invisible to
/// the visitor. Measured, that shape suppresses the lint for the expanded
/// module while Clippy's own guard stays silent, so the tokens are walked here
/// as well. Groups are entered recursively, since the attribute sits inside
/// the arm's braces.
fn suppressed_in_tokens(tokens: TokenStream) -> Vec<(String, String)> {
    let trees: Vec<TokenTree> = tokens.into_iter().collect();
    let mut found = Vec::new();

    for (index, tree) in trees.iter().enumerate() {
        match tree {
            TokenTree::Group(group) => found.extend(suppressed_in_tokens(group.stream())),
            TokenTree::Punct(punct) if punct.as_char() == '#' => {
                found.extend(attribute_at(&trees, index));
            }
            TokenTree::Punct(_) | TokenTree::Ident(_) | TokenTree::Literal(_) => {}
        }
    }
    found
}

/// Return the protected lints suppressed by the attribute starting at `index`.
///
/// `index` addresses the `#`. What follows is an optional `!`, which makes the
/// attribute inner, and then the bracketed meta.
fn attribute_at(trees: &[TokenTree], index: usize) -> Vec<(String, String)> {
    let (scope, cursor) = scope_after_hash(trees, index);
    let Some(group) = bracketed_group(trees, cursor) else {
        return Vec::new();
    };
    let Ok(meta) = syn::parse2::<Meta>(group.stream()) else {
        return Vec::new();
    };

    let bang = if scope == Scope::Inner { "!" } else { "" };
    let rendered = format!("#{bang}[{}] (in a macro body)", group.stream());
    lints_from_meta(&meta, scope)
        .into_iter()
        .map(|lint| (lint, rendered.clone()))
        .collect()
}

/// Return the scope the token after a `#` implies, and the index after it.
fn scope_after_hash(trees: &[TokenTree], index: usize) -> (Scope, usize) {
    let cursor = index.saturating_add(1);
    match trees.get(cursor) {
        Some(TokenTree::Punct(bang)) if bang.as_char() == '!' => {
            (Scope::Inner, cursor.saturating_add(1))
        }
        _ => (Scope::Outer, cursor),
    }
}

/// Return the bracketed group at `cursor`, if there is one.
fn bracketed_group(trees: &[TokenTree], cursor: usize) -> Option<&proc_macro2::Group> {
    match trees.get(cursor) {
        Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::Bracket => Some(group),
        _ => None,
    }
}

/// Render an attribute roughly as written, for a failure message.
fn render_attribute(attribute: &Attribute) -> String {
    let bang = match attribute.style {
        AttrStyle::Inner(_) => "!",
        AttrStyle::Outer => "",
    };
    let path = render_path(attribute.path());
    attribute.meta.require_list().map_or_else(
        |_| format!("#{bang}[{path}]"),
        |list| format!("#{bang}[{path}({})]", list.tokens),
    )
}

/// Return every protected lint suppressed in one source file.
fn suppressed_lints(contents: &str) -> TestResult<Vec<(String, String)>> {
    let parsed = syn::parse_file(contents)?;
    let mut collector = AttributeCollector::default();
    collector.visit_file(&parsed);

    let from_attributes = collector.attributes.iter().flat_map(|attribute| {
        suppressed_by(attribute)
            .into_iter()
            .map(|lint| (lint, render_attribute(attribute)))
    });
    let from_macros = collector
        .macro_tokens
        .into_iter()
        .flat_map(suppressed_in_tokens);

    Ok(from_attributes
        .chain(from_macros)
        .filter(|(lint, _)| PROTECTED_LINTS.contains(&lint.as_str()))
        .collect())
}

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
    let sanctioned = concat!(
        "#[expect(clippy::disallowed_methods, reason = \"composition root\")]\n",
        "fn read() -> Option<String> { std::env::var(\"HOME\").ok() }\n",
    );

    assert!(suppressed_lints(sanctioned)?.is_empty());
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
    let nested = "#![cfg_attr(all(), allow(clippy::disallowed_methods, reason = \"x\"))]\n";

    assert_eq!(suppressed_lints(nested)?.len(), 1);
    Ok(())
}

/// Scenario: the suppression names the lint's group rather than the lint.
///
/// Invariant: it is reported. Clippy places `disallowed_methods` in `style`,
/// so `#![allow(clippy::style)]` switches the policy off without ever naming
/// it.
#[test]
fn a_suppression_of_the_lints_group_is_an_offence() -> TestResult {
    assert_eq!(suppressed_lints("#![allow(clippy::style)]\n")?.len(), 1);
    Ok(())
}

/// Scenario: awkward spacing, and a parenthesis inside the reason string.
///
/// Invariant: both are handled. A scan matching a fixed opener would miss the
/// spaced form, and one counting raw parentheses would end the attribute early
/// at the parenthesis inside the string.
#[test]
fn spacing_and_a_parenthesis_in_the_reason_do_not_hide_a_suppression() -> TestResult {
    let awkward = "#![allow (warnings, reason = \"see the note (below)\")]\n";

    assert_eq!(suppressed_lints(awkward)?.len(), 1);
    Ok(())
}

/// Scenario: attribute-shaped text inside a string literal or a doc comment.
///
/// Invariant: neither is an offence. This is the false positive a text scan
/// cannot avoid, and a contract that reports one gets disabled. This file's
/// own documentation quotes the attribute repeatedly.
#[test]
fn attribute_shaped_text_is_not_an_attribute() -> TestResult {
    let literal = concat!(
        "//! Never write #![allow(clippy::disallowed_methods)] in a crate root.\n",
        "const EXAMPLE: &str = \"\n#![allow(warnings)]\n\";\n",
    );

    assert!(suppressed_lints(literal)?.is_empty());
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
    let innocent = "#[allow(clippy::alloc_instead_of_core)]\nfn documented() {}\n";

    assert!(suppressed_lints(innocent)?.is_empty());
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
    let crate_scoped = "#![expect(clippy::disallowed_methods, reason = \"x\")]\n";

    assert_eq!(suppressed_lints(crate_scoped)?.len(), 1);
    Ok(())
}

/// Scenario: a crate-scoped `expect` reached through a `cfg_attr`.
///
/// Invariant: it is reported too. The scope of the outermost attribute is
/// carried through the nesting, so the conditional form is judged as the
/// crate-scoped expectation it becomes.
#[test]
fn a_crate_scoped_expect_nested_in_cfg_attr_is_an_offence() -> TestResult {
    let nested = "#![cfg_attr(all(), expect(clippy::disallowed_methods))]\n";

    assert_eq!(suppressed_lints(nested)?.len(), 1);
    Ok(())
}

/// Scenario: the attribute or the lint is spelled as a raw identifier.
///
/// Invariant: both are reported. `r#allow` and `clippy::r#style` name exactly
/// the same attribute and lint as their plain spellings, and both were
/// measured to suppress the policy lint, but `syn` renders an identifier with
/// its `r#` prefix intact. Normalizing before comparison is what catches them.
#[test]
fn a_raw_identifier_does_not_hide_a_suppression() -> TestResult {
    for raw in [
        "#![r#allow(clippy::disallowed_methods)]\n",
        "#![allow(clippy::r#style)]\n",
        "#![allow(r#warnings)]\n",
    ] {
        assert_eq!(suppressed_lints(raw)?.len(), 1, "missed {raw}");
    }
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
    let shadowed = concat!(
        "macro_rules! shadow {\n",
        "    () => {\n",
        "        mod inner {\n",
        "            #![allow(clippy::disallowed_methods)]\n",
        "            pub fn call() { let _ = std::env::var(\"A\"); }\n",
        "        }\n",
        "    };\n",
        "}\n",
    );

    assert_eq!(suppressed_lints(shadowed)?.len(), 1);
    Ok(())
}
