//! What the scan must catch, and what it must not report.
//!
//! The file beside this one runs the scan over the workspace. This binary
//! holds the probes: one file per shape, under
//! `backend/tests/fixtures/environment_policy_scan/`, each carrying the
//! `.rs.txt` suffix so the scan does not read its own counterexamples as
//! sources. Every shape was measured against this repository's own Clippy
//! before it was added; the measurements and the mutation record live in
//! `environment_policy_source_scan.rs`.
//!
//! Roughly half the cases are negative. A contract that reports a false
//! positive gets switched off, so each rule carries the case that keeps it
//! narrow beside the case that proves it reaches.
//!
//! The cases are split in two along the question they ask. The attribute
//! cases ask whether an attribute suppresses a protected lint; the inclusion
//! cases ask whether a construct puts Rust past the traversal unread. Kept in
//! one file they were over the 400-line limit, and the two halves share
//! nothing but the fixture directory.
//!
//! The group directory is named apart from this file on purpose. Clippy's
//! `self_named_module_files` refuses a directory beside a file of the same
//! name, so `environment_policy_scan_cases.rs` cannot own an
//! `environment_policy_scan_cases/`; the modules are therefore declared with
//! `#[path]` out of a directory named for what it holds.

#[expect(
    dead_code,
    reason = "these cases drive the judgement only; the workspace reading is \
              exercised by environment_policy_source_scan.rs"
)]
mod environment_policy_scan;

#[path = "environment_policy_scan_case_groups/attribute_cases.rs"]
mod attribute_cases;
#[path = "environment_policy_scan_case_groups/inclusion_cases.rs"]
mod inclusion_cases;
