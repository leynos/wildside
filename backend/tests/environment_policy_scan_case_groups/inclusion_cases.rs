//! Which `include!` targets the scan can follow, and which it cannot.
//!
//! Separated from the attribute cases because the rule is a different one:
//! those ask whether an attribute suppresses a protected lint, these ask
//! whether a construct puts Rust past the traversal unread. The probes and
//! their `.rs.txt` suffix are described beside the attribute cases.

use std::path::Path as StdPath;

use crate::environment_policy_scan::{TestResult, unreadable_includes};

/// The file the probes are judged as if they were.
///
/// An `include!` target is resolved against the file that includes it, so the
/// cases need a source to be. `backend/tests` is where this workspace's own
/// `include!` calls live, and where a probe would sit if it were real.
const PROBE: &str = "backend/tests/probe.rs";

/// An `include!` of a target the scan does not read.
const INCLUDE_UNREADABLE: &str =
    include_str!("../fixtures/environment_policy_scan/include_unreadable.rs.txt");
/// An `include!` naming a `.rs` file, the form this workspace uses.
const INCLUDE_RUST_PATH: &str =
    include_str!("../fixtures/environment_policy_scan/include_rust_path.rs.txt");
/// An `include!` whose target is computed rather than named.
const INCLUDE_COMPUTED: &str =
    include_str!("../fixtures/environment_policy_scan/include_computed.rs.txt");
/// One `.rs` target, written as a raw string and with an escaped dot.
const INCLUDE_SPELLINGS: &str =
    include_str!("../fixtures/environment_policy_scan/include_spellings.rs.txt");
/// An `include!` of a bare extension, which names no file the scan collects.
const INCLUDE_BARE_EXTENSION: &str =
    include_str!("../fixtures/environment_policy_scan/include_bare_extension.rs.txt");
/// A `macro_rules!` arm whose expansion includes an unreadable target.
const MACRO_INCLUDE_UNREADABLE: &str =
    include_str!("../fixtures/environment_policy_scan/macro_include_unreadable.rs.txt");
/// Two arms: one including a collected file, one embedding data.
const MACRO_INCLUDE_READABLE: &str =
    include_str!("../fixtures/environment_policy_scan/macro_include_readable.rs.txt");
/// A target whose separator is a backslash, which is two things at once.
const INCLUDE_BACKSLASH_TARGET: &str =
    include_str!("../fixtures/environment_policy_scan/include_backslash_target.rs.txt");
/// Targets that are `.rs` and outside the tree the traversal walks.
const INCLUDE_ESCAPING_ROOT: &str =
    include_str!("../fixtures/environment_policy_scan/include_escaping_root.rs.txt");
/// `include_str!` and `include_bytes!`, which embed data rather than source.
const INCLUDE_STR_IS_DATA: &str =
    include_str!("../fixtures/environment_policy_scan/include_str_is_data.rs.txt");

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
    assert_eq!(
        unreadable_includes(INCLUDE_UNREADABLE, StdPath::new(PROBE))?.len(),
        1
    );
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
    assert_eq!(
        unreadable_includes(INCLUDE_COMPUTED, StdPath::new(PROBE))?.len(),
        1
    );
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
        unreadable_includes(INCLUDE_RUST_PATH, StdPath::new(PROBE))?.is_empty(),
        "the workspace's own include! form was reported"
    );
    Ok(())
}

/// Scenario: one `.rs` target, written as a raw string and with the dot
/// escaped as `\x2E`.
///
/// Invariant: neither is an offence. rustc resolves the decoded value, so
/// both name `support/entrypoint.rs`, a file the scan already reads. A rule
/// that judged the literal as it is written would report a target it can
/// read perfectly well, and the first author to write a Windows path as a
/// raw string would meet a finding with no defect behind it.
#[test]
fn the_spelling_of_a_rust_target_does_not_matter() -> TestResult {
    assert!(
        unreadable_includes(INCLUDE_SPELLINGS, StdPath::new(PROBE))?.is_empty(),
        "a raw or escaped spelling of a .rs target was reported"
    );
    Ok(())
}

/// Scenario: `include!(".rs")`, a bare extension with no file stem.
///
/// Invariant: it is an offence. This is the one input on which
/// `ends_with(".rs")` and the traversal disagree: the text ends with `.rs`,
/// but `Path::new(".rs")` has no extension at all, `.rs` being the whole
/// stem, so `rust_sources` would never collect such a file and whatever
/// rustc included behind it would go unread. The rule therefore compares the
/// extension exactly as the traversal selects sources, through one shared
/// constant.
#[test]
fn an_include_of_a_bare_extension_is_an_offence() -> TestResult {
    assert_eq!(
        unreadable_includes(INCLUDE_BARE_EXTENSION, StdPath::new(PROBE))?.len(),
        1
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
        unreadable_includes(INCLUDE_STR_IS_DATA, StdPath::new(PROBE))?.is_empty(),
        "include_str! or include_bytes! was read as source inclusion"
    );
    Ok(())
}

/// Scenario: an `include!` inside a `macro_rules!` body.
///
/// Invariant: it is reported. `syn` leaves a macro body opaque, so the arm's
/// `include!` is not a `Macro` node the visitor ever sees, exactly as an
/// inner attribute in an arm is not. rustc parses the target as Rust on
/// expansion all the same, so a suppression at the top of that file reaches
/// the calls around the invocation. The attribute walk already descends into
/// token streams for this reason; the inclusion rule now does too.
#[test]
fn an_include_inside_a_macro_body_is_an_offence() -> TestResult {
    assert_eq!(
        unreadable_includes(MACRO_INCLUDE_UNREADABLE, StdPath::new(PROBE))?.len(),
        1
    );
    Ok(())
}

/// Scenario: a macro arm including a collected file, and one embedding data.
///
/// Invariant: neither is an offence. The token walk finds the call, and the
/// same judgement the syntax-tree pass uses decides the target, so a macro
/// that includes `./support/entrypoint.rs` is as clean inside an arm as it is
/// outside one. `include_str!` is a different identifier and is not a call
/// this rule recognizes at all. Without this case the token walk could report
/// every macro in the workspace that touches a file.
#[test]
fn a_macro_including_a_collected_file_is_not_an_offence() -> TestResult {
    assert!(
        unreadable_includes(MACRO_INCLUDE_READABLE, StdPath::new(PROBE))?.is_empty(),
        "a macro arm including a collected file, or embedding data, was reported"
    );
    Ok(())
}

/// Scenario: `.rs` targets that lie outside the roots the scan walks.
///
/// Invariant: both are reported. `rust_sources` walks `SOURCE_ROOTS` and
/// nothing else, so `../third_party/vendor.rs` and `/etc/vendor.rs` name real
/// Rust files the scan never reads, and an inner suppression at the top of
/// either reaches the including source unseen. The extension was the whole
/// test before, which accepted any existing `.rs` file anywhere on the
/// machine. The judgement stays lexical: a target that escapes the tree is a
/// finding whether or not the file it names exists today.
#[test]
fn an_include_escaping_the_scanned_roots_is_an_offence() -> TestResult {
    assert_eq!(
        unreadable_includes(INCLUDE_ESCAPING_ROOT, StdPath::new(PROBE))?.len(),
        2
    );
    Ok(())
}

/// Scenario: a target spelled with a backslash separator.
///
/// Invariant: it is reported. On Unix the whole string is one file name, so
/// the traversal collects nothing called `support\\entrypoint.rs` and a
/// lexical judgement that split it would be judging a path this platform
/// does not have; on Windows it is two components and the same text names a
/// different file. A target that means two things cannot be shown to name a
/// collected source, so it is a finding on both. Without this case the
/// backslash refusal could be deleted and every other inclusion case would
/// still pass.
#[test]
fn an_include_spelled_with_a_backslash_is_an_offence() -> TestResult {
    assert_eq!(
        unreadable_includes(INCLUDE_BACKSLASH_TARGET, StdPath::new(PROBE))?.len(),
        1
    );
    Ok(())
}
