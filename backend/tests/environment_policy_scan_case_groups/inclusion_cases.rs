//! Which `include!` targets the scan can follow, and which it cannot.
//!
//! Separated from the attribute cases because the rule is a different one:
//! those ask whether an attribute suppresses a protected lint, these ask
//! whether a construct puts Rust past the traversal unread. The probes and
//! their `.rs.txt` suffix are described beside the attribute cases.

use crate::environment_policy_scan::{TestResult, unreadable_includes};

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
        unreadable_includes(INCLUDE_SPELLINGS)?.is_empty(),
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
    assert_eq!(unreadable_includes(INCLUDE_BARE_EXTENSION)?.len(), 1);
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
