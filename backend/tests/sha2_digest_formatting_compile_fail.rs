//! Compile-fail coverage for the `sha2` 0.11 API break.
//!
//! The runtime tests around `sha256_file` confirm the digests this crate
//! produces, but they cannot detect a reintroduction of the pre-0.11 patterns
//! elsewhere. These fixtures pin the two source-breaking changes the migration
//! worked around: `{:x}` formatting of a finalized digest, and streaming into a
//! hasher through `std::io::Write`. Should either start compiling — for example
//! because `sha2` was downgraded to 0.10 — this test fails and flags the
//! regression.
//!
//! Gated behind the `trybuild-tests` feature so ordinary `cargo test` runs stay
//! fast; `make test` and CI enable it via `--all-features`.

#[cfg(feature = "trybuild-tests")]
#[test]
fn sha2_pre_migration_digest_patterns_do_not_compile() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/support/ui/sha2_digest_lowerhex.rs");
    t.compile_fail("tests/support/ui/sha2_hasher_io_write.rs");
}
