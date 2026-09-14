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
use proc_macro2::TokenStream;
use syn::LitStr;
use syn::visit::Visit;

mod attributes;

use attributes::{
    AttributeCollector, render_attribute, render_path, suppressed_by, suppressed_in_tokens,
};

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

/// Spellings of the macro that makes rustc parse another file as Rust.
///
/// `include_str!` and `include_bytes!` are absent deliberately: they embed a
/// file as data, so an attribute inside one is text and suppresses nothing.
/// This scan's own probe fixtures are loaded that way.
const INCLUDING_MACROS: [&str; 3] = ["include", "std::include", "core::include"];

/// Return every `include!` whose target this scan cannot read.
///
/// rustc parses an `include!` target as Rust whatever its extension, so an
/// `#![allow(clippy::disallowed_methods)]` at the top of an included
/// `.rs.txt` silences the calls around it while the scan, which reads only
/// `.rs` files, never sees the attribute. Measured against this repository's
/// Clippy on 2026-09-14: a file whose whole body was `include!` of such a
/// target took a prohibited call from one diagnostic to none.
///
/// The rule is therefore about reachability rather than content: an
/// `include!` is a finding unless its target is a string literal ending in
/// `.rs`, because only then is the included source a file the scan already
/// reads. Every `include!` in this workspace names `support/entrypoint.rs`
/// literally, so the rule costs nothing today and closes the route.
///
/// A computed target, such as the `include!(concat!(env!("OUT_DIR"), ...))`
/// that build scripts use, is a finding too. That is not an accusation: it is
/// the honest report that the scan cannot know what was included, and the
/// remedy is to name the file.
pub(crate) fn unreadable_includes(contents: &str) -> TestResult<Vec<String>> {
    let parsed = syn::parse_file(contents)?;
    let mut collector = AttributeCollector::default();
    collector.visit_file(&parsed);

    Ok(collector
        .macros
        .iter()
        .filter(|mac| INCLUDING_MACROS.contains(&render_path(&mac.path).as_str()))
        .filter(|mac| !names_a_rust_file(&mac.tokens))
        .map(|mac| format!("include!({})", mac.tokens))
        .collect())
}

/// Whether a macro's tokens are a string literal naming a `.rs` file.
fn names_a_rust_file(tokens: &TokenStream) -> bool {
    syn::parse2::<LitStr>(tokens.clone()).is_ok_and(|literal| literal.value().ends_with(".rs"))
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
            .macros
            .iter()
            .flat_map(|mac| suppressed_in_tokens(mac.tokens.clone())),
    );

    found.retain(|(lint, _)| PROTECTED_LINTS.contains(&lint.as_str()));
    Ok(found)
}
