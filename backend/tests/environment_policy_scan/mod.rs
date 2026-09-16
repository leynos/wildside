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
use std::path::{Component, Path as StdPath, PathBuf};

use cap_std::ambient_authority;
use cap_std::fs::Dir;
use proc_macro2::TokenStream;
use syn::LitStr;
use syn::visit::Visit;

mod attributes;

use attributes::{
    AttributeCollector, includes_in_tokens, render_attribute, render_path, suppressed_by,
    suppressed_in_tokens,
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

/// The extension of the sources this scan reads.
///
/// Shared by the traversal and by the `include!` rule so the two cannot
/// drift. The rule's whole claim is that an accepted target is a file the
/// traversal already collects, and two separate spellings of "is it Rust"
/// is exactly how that claim stops being true.
pub(crate) const SOURCE_EXTENSION: &str = "rs";

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
            } else if path
                .extension()
                .is_some_and(|extension| extension == SOURCE_EXTENSION)
            {
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
/// `include!` is a finding unless its target is a string literal naming a
/// file the traversal already collects, which is what
/// [`names_a_collected_source`] decides. Every `include!` in this workspace
/// names a `.rs` file inside `backend/tests`, so the rule costs nothing today
/// and closes the route.
///
/// A computed target, such as the `include!(concat!(env!("OUT_DIR"), ...))`
/// that build scripts use, is a finding too. That is not an accusation: it is
/// the honest report that the scan cannot know what was included, and the
/// remedy is to name the file.
///
/// Macro bodies are searched as well as the syntax tree, for the reason the
/// attribute walk gives: `syn` leaves a macro body opaque, so a
/// `macro_rules!` arm expanding to an `include!` is a call the visitor never
/// sees, and rustc parses its target on expansion all the same. Discovery
/// happens in [`includes_in_tokens`]; the judgement of a discovered target is
/// the one below, so the two passes cannot disagree about what is readable.
///
/// `including` is the file the targets are resolved against. It is the path
/// `rust_sources` yields, relative to the workspace root, so a target that
/// climbs out of [`SOURCE_ROOTS`] is visible as such without a filesystem
/// call.
pub(crate) fn unreadable_includes(contents: &str, including: &StdPath) -> TestResult<Vec<String>> {
    let parsed = syn::parse_file(contents)?;
    let mut collector = AttributeCollector::default();
    collector.visit_file(&parsed);

    Ok(collector
        .macros
        .iter()
        .flat_map(|mac| {
            let direct = INCLUDING_MACROS
                .contains(&render_path(&mac.path).as_str())
                .then(|| mac.tokens.clone());
            direct
                .into_iter()
                .chain(includes_in_tokens(mac.tokens.clone()))
        })
        .filter(|arguments| !names_a_collected_source(arguments, including))
        .map(|arguments| format!("include!({arguments})"))
        .collect())
}

/// Whether a macro's tokens name a file the workspace traversal collects.
///
/// Three decisions, and each closes a gap between this rule and the traversal
/// it stands in for.
///
/// The tokens are parsed as one `LitStr` and judged by their decoded value,
/// not by how they are written. `r"support/entrypoint.rs"` and
/// `"support/entrypoint\x2Ers"` name the same file rustc will include, so a
/// rule reading the source spelling would report a target it can perfectly
/// well read. Parsing also supplies the strictness: `parse2` refuses a
/// `concat!` and refuses trailing tokens, so a computed target stays a
/// finding.
///
/// The extension is then compared the way [`rust_sources`] selects files,
/// through the shared [`SOURCE_EXTENSION`], rather than with
/// `ends_with(".rs")`. Those two disagree on exactly one input, and it is a
/// hole: `include!(".rs")` ends with `.rs` and has no extension at all,
/// `.rs` being the whole file stem, so the traversal would never collect it
/// and the rule would wave it through. Comparing an `OsStr` for equality is
/// also case-sensitive without the text match Clippy's
/// `case_sensitive_file_extension_comparisons` rejects.
///
/// The extension alone is still not the whole claim. `rust_sources` walks
/// [`SOURCE_ROOTS`] and nothing else, so `include!("../third_party/vendor.rs")`
/// names a real `.rs` file the scan never reads, and an inner suppression at
/// the top of that file reaches the including source unseen. The target is
/// therefore required to be lexically inside the tree that includes it:
/// resolved against the file that includes it and required to land inside
/// [`SOURCE_ROOTS`]. A backslash is refused outright, being an ordinary
/// character in a Unix file name and a separator on Windows. The resolution
/// is by text rather than by the filesystem: canonicalizing would make the
/// judgement depend on which files happen to exist on the machine running
/// the scan, and a target that escapes the scanned tree is a finding whether
/// or not the file it names exists today. Every `include!` in this workspace
/// names `support/entrypoint.rs`, so the rule costs nothing here.
fn names_a_collected_source(tokens: &TokenStream, including: &StdPath) -> bool {
    syn::parse2::<LitStr>(tokens.clone())
        .is_ok_and(|literal| is_collected_source(&literal.value(), including))
}

/// Whether one decoded `include!` target lies inside the scanned tree.
///
/// A backslash is refused outright. It is an ordinary character in a Unix
/// file name and a separator on Windows, so a target carrying one means two
/// different things on the two platforms and only one of them is checkable
/// here.
fn is_collected_source(target: &str, including: &StdPath) -> bool {
    if target.contains('\\') {
        return false;
    }
    let path = StdPath::new(target);
    if !path
        .extension()
        .is_some_and(|extension| extension == SOURCE_EXTENSION)
    {
        return false;
    }
    let base = including.parent().unwrap_or_else(|| StdPath::new(""));
    lexically_resolved(base, path)
        .is_some_and(|resolved| SOURCE_ROOTS.iter().any(|root| resolved.starts_with(root)))
}

/// Join a target onto its including directory, resolving `.` and `..` by text.
///
/// No filesystem call is made. Resolving the path on disk would make the
/// judgement depend on which files happen to exist, and a target that escapes
/// the scanned tree is a finding whether or not it resolves today. A `..`
/// that climbs above the including directory yields None, as does an absolute
/// path or a Windows prefix: each names somewhere the traversal never walks.
fn lexically_resolved(base: &StdPath, target: &StdPath) -> Option<PathBuf> {
    let mut resolved = PathBuf::new();
    for component in base.components().chain(target.components()) {
        match component {
            Component::Normal(part) => resolved.push(part),
            Component::CurDir => {}
            Component::ParentDir if resolved.pop() => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(resolved)
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
