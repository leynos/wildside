//! Judging one attribute, and the token streams a macro hides.
//!
//! Split from the module beside it when the two new routes of 2026-09-14
//! carried it past the 400-line limit `AGENTS.md` sets. The seam is between
//! reading the workspace and deciding what one attribute means: everything
//! here works on a parsed file and knows nothing about the filesystem.

use proc_macro2::{Delimiter, Ident, TokenStream, TokenTree};
use syn::ext::IdentExt;
use syn::punctuated::Punctuated;
use syn::visit::Visit;
use syn::{AttrStyle, Attribute, Macro, Meta, MetaList, Path, Token};

use super::TestResult;

/// Collect every attribute in a parsed file, and every macro token stream.
///
/// A visitor is used rather than a hand-rolled walk so attributes on nested
/// items, on function-local items, and on expressions are all reached. Macro
/// bodies are opaque to the visitor, so their token streams are collected for
/// a separate pass: a `macro_rules!` arm expanding to a module with an inner
/// `allow` would otherwise hide a suppression from both this scan and Clippy's
/// own guard.
#[derive(Default)]
pub(super) struct AttributeCollector {
    pub(super) attributes: Vec<Attribute>,
    pub(super) macros: Vec<Macro>,
}

impl<'ast> Visit<'ast> for AttributeCollector {
    fn visit_attribute(&mut self, attribute: &'ast Attribute) {
        self.attributes.push(attribute.clone());
    }

    fn visit_macro(&mut self, mac: &'ast Macro) {
        self.macros.push(mac.clone());
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
pub(super) fn render_path(path: &Path) -> String {
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
pub(super) fn suppressed_by(attribute: &Attribute) -> TestResult<Vec<String>> {
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
pub(super) fn suppressed_in_tokens(tokens: TokenStream) -> Vec<(String, String)> {
    let trees: Vec<TokenTree> = tokens.into_iter().collect();
    let mut found = Vec::new();
    let mut index = 0;

    while index < trees.len() {
        match &trees[index] {
            TokenTree::Punct(punct) if punct.as_char() == '#' => {
                let (lints, next) = attribute_at(&trees, index);
                found.extend(lints);
                index = next;
            }
            TokenTree::Ident(ident) => {
                found.extend(forwarded_at(&trees, index, ident));
                index = index.saturating_add(1);
            }
            TokenTree::Group(group) => {
                found.extend(suppressed_in_tokens(group.stream()));
                index = index.saturating_add(1);
            }
            TokenTree::Punct(_) | TokenTree::Literal(_) => index = index.saturating_add(1),
        }
    }
    found
}

/// Return the argument tokens of every `include!` inside a token stream.
///
/// The same reason the attribute walk above exists: `syn` leaves a macro body
/// opaque, so a `macro_rules!` arm expanding to `include!("probe.rs.txt")` is
/// a `Macro` node the visitor never sees. rustc parses that target as Rust on
/// expansion, so an `#![allow(...)]` at its top silences the calls around it
/// while a scan reading only the syntax tree's macro nodes finds nothing.
///
/// Only discovery happens here. Whether a discovered target is one the
/// traversal already collects is the caller's judgement, stated once in
/// [`super::names_a_collected_source`], so the two cannot drift apart.
///
/// The spellings are the same three the caller accepts, because `std::include!`
/// and `core::include!` reach rustc identically. `include_str!` is not among
/// them: the `!` must follow the last path segment, so the longer name is a
/// different identifier and never matches.
pub(super) fn includes_in_tokens(tokens: TokenStream) -> Vec<TokenStream> {
    let trees: Vec<TokenTree> = tokens.into_iter().collect();
    let mut found = Vec::new();

    for index in 0..trees.len() {
        if let TokenTree::Group(group) = &trees[index] {
            found.extend(includes_in_tokens(group.stream()));
        }
        if let Some(arguments) = include_at(&trees, index) {
            found.push(arguments);
        }
    }
    found
}

/// Return an `include!` invocation's arguments when one starts at `index`.
///
/// A call is an identifier spelling the macro, optionally preceded by path
/// segments, then `!`, then a delimited group. The identifier is unraw'd, so
/// `r#include!` is the same call.
fn include_at(trees: &[TokenTree], index: usize) -> Option<TokenStream> {
    let TokenTree::Ident(ident) = &trees[index] else {
        return None;
    };
    if ident.unraw() != "include" {
        return None;
    }
    let TokenTree::Punct(punct) = trees.get(index.saturating_add(1))? else {
        return None;
    };
    if punct.as_char() != '!' {
        return None;
    }
    let TokenTree::Group(group) = trees.get(index.saturating_add(2))? else {
        return None;
    };
    Some(group.stream())
}

/// Return the protected lints a forwarded suppression names at `index`.
///
/// `macro_rules! forward { ($a:meta) => { #[$a] fn call() {...} } }` hides
/// half a suppression, and `forward!(allow(clippy::disallowed_methods))` hides
/// the other: neither is an attribute anything here can judge, and together
/// they silence the call. Measured against this repository's Clippy on
/// 2026-09-14, that pair took the prohibited call from one diagnostic to none.
///
/// The transcriber's `#[$a]` cannot be judged, because what it becomes is not
/// in the source. The invocation's argument can be, and it is the half that
/// names the lint. So a bare `allow(...)` or `expect(...)` inside a macro's
/// tokens is judged exactly as the attribute it becomes.
///
/// Narrowness is what keeps this usable: only a protected lint is reported, so
/// `forward!(allow(dead_code))` and a method named `allow` are untouched. A
/// real attribute never reaches here, because the `#` arm consumes its
/// brackets before this arm is tried.
fn forwarded_at(trees: &[TokenTree], index: usize, ident: &Ident) -> Vec<(String, String)> {
    let keyword = ident.unraw().to_string();
    if keyword != "allow" && keyword != "expect" {
        return Vec::new();
    }
    let cursor = index.saturating_add(1);
    let Some(TokenTree::Group(group)) = trees.get(cursor) else {
        return Vec::new();
    };
    if group.delimiter() != Delimiter::Parenthesis {
        return Vec::new();
    }
    let meta = format!("{keyword}{group}");
    let Ok(parsed) = syn::parse_str::<Meta>(&meta) else {
        return Vec::new();
    };
    // An attribute forwarded through a macro carries no scope of its own, so
    // it is judged as the inner form, the one that suppresses the most.
    let Ok(lints) = lints_from_meta(&parsed, Scope::Inner) else {
        return Vec::new();
    };
    let rendered = format!("{meta} (forwarded through a macro)");
    lints
        .into_iter()
        .map(|lint| (lint, rendered.clone()))
        .collect()
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
fn attribute_at(trees: &[TokenTree], index: usize) -> (Vec<(String, String)>, usize) {
    let (scope, cursor) = scope_after_hash(trees, index);
    let after = cursor.saturating_add(1);
    let Some(group) = bracketed_group(trees, cursor) else {
        return (Vec::new(), index.saturating_add(1));
    };
    let Ok(meta) = syn::parse2::<Meta>(group.stream()) else {
        return (Vec::new(), after);
    };
    let Ok(lints) = lints_from_meta(&meta, scope) else {
        return (Vec::new(), after);
    };

    let bang = if scope == Scope::Inner { "!" } else { "" };
    let rendered = format!("#{bang}[{}] (in a macro body)", group.stream());
    let found = lints
        .into_iter()
        .map(|lint| (lint, rendered.clone()))
        .collect();
    (found, after)
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
pub(super) fn render_attribute(attribute: &Attribute) -> String {
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
