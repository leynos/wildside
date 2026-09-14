//! The scan itself: reading the workspace's sources and judging attributes.
//!
//! Split out of `environment_policy_source_scan.rs`, which crossed the
//! 400-line limit `AGENTS.md` sets. The seam is the obvious one: this module
//! is the mechanism, and the test file beside it is what the mechanism has to
//! catch and what it must not report.
//!
//! Everything here is `pub(crate)` rather than private, because the test
//! binary is the only consumer and an unused-item warning is not worth
//! silencing individually.

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

pub(crate) type TestResult<T = ()> = Result<T, Box<dyn StdError>>;

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
pub(crate) const PROTECTED_LINTS: [&str; 7] = [
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
pub(crate) const SOURCE_ROOTS: [&str; 3] = ["backend", "crates", "tools"];

/// Return the workspace root, from this crate's manifest directory.
pub(crate) fn workspace_root() -> TestResult<PathBuf> {
    Ok(StdPath::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("the backend manifest directory must have a parent")?
        .to_path_buf())
}

/// Collect every `.rs` file under one root, depth first.
///
/// Paths are reported relative to the workspace root, because that is what a
/// contributor acting on a failure needs to open.
pub(crate) fn rust_sources(root: &StdPath, relative: &str) -> TestResult<Vec<(PathBuf, String)>> {
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
///
/// A failure to parse the arguments is returned rather than swallowed. The
/// attribute reached here came from a file the compiler accepted, so an
/// argument list this cannot read is an anomaly, and reporting it as "no
/// lints suppressed" would be the quietest possible bypass. The macro-body
/// pass has its own rule, because a metavariable is not an anomaly there.
fn allowed_lints(list: &MetaList) -> TestResult<Vec<String>> {
    let nested = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
    Ok(nested
        .iter()
        .filter_map(|meta| match meta {
            Meta::Path(path) => Some(render_path(path)),
            Meta::List(_) | Meta::NameValue(_) => None,
        })
        .collect())
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
fn suppressed_by_cfg_attr(list: &MetaList, scope: Scope) -> TestResult<Vec<String>> {
    let nested = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
    let mut lints = Vec::new();
    for meta in nested.iter().skip(1) {
        lints.extend(lints_from_meta(meta, scope)?);
    }
    Ok(lints)
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
fn lints_from_meta(meta: &Meta, scope: Scope) -> TestResult<Vec<String>> {
    let Meta::List(list) = meta else {
        return Ok(Vec::new());
    };
    match render_path(&list.path).as_str() {
        "allow" => allowed_lints(list),
        "expect" if scope == Scope::Inner => allowed_lints(list),
        "cfg_attr" => suppressed_by_cfg_attr(list, scope),
        _ => Ok(Vec::new()),
    }
}

/// Return the lint names one attribute suppresses.
fn suppressed_by(attribute: &Attribute) -> TestResult<Vec<String>> {
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
/// as well. Groups are entered recursively, so the attribute is reached inside
/// the arm's braces and inside a second `macro_rules!` nested within them;
/// both depths were measured to take the prohibited call from one diagnostic
/// to none.
///
/// Two limits are worth stating. A procedural macro that synthesizes the
/// attribute is out of reach, because its output does not exist in the source.
/// And `cfg` conditions are not evaluated here, any more than in the
/// syntax-tree pass.
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
///
/// A shape here that does not parse is not an anomaly, unlike one in the
/// syntax-tree pass. A macro arm legitimately writes `#[$attribute]` or
/// `#[allow($lint)]`, whose metavariables are not a `Meta` and never will be
/// until the macro is expanded. Reporting those would make every
/// code-generating macro a finding, and a contract that reports false
/// positives gets switched off. They are therefore not suppressions this scan
/// can judge, and the negative case is covered by a regression test.
fn attribute_at(trees: &[TokenTree], index: usize) -> Vec<(String, String)> {
    let (scope, cursor) = scope_after_hash(trees, index);
    let Some(group) = bracketed_group(trees, cursor) else {
        return Vec::new();
    };
    let Ok(meta) = syn::parse2::<Meta>(group.stream()) else {
        return Vec::new();
    };
    let Ok(lints) = lints_from_meta(&meta, scope) else {
        return Vec::new();
    };

    let bang = if scope == Scope::Inner { "!" } else { "" };
    let rendered = format!("#{bang}[{}] (in a macro body)", group.stream());
    lints
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
pub(crate) fn suppressed_lints(contents: &str) -> TestResult<Vec<(String, String)>> {
    let parsed = syn::parse_file(contents)?;
    let mut collector = AttributeCollector::default();
    collector.visit_file(&parsed);

    let mut found = Vec::new();
    for attribute in &collector.attributes {
        let rendered = render_attribute(attribute);
        found.extend(
            suppressed_by(attribute)?
                .into_iter()
                .map(|lint| (lint, rendered.clone())),
        );
    }
    found.extend(
        collector
            .macro_tokens
            .into_iter()
            .flat_map(suppressed_in_tokens),
    );

    found.retain(|(lint, _)| PROTECTED_LINTS.contains(&lint.as_str()));
    Ok(found)
}
