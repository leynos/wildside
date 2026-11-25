//! Trybuild coverage for the retry API's compile-time surface.
//!
//! The compile-pass fixture is the assertion that `JitterSample::midpoint`
//! stays callable in a `const` item: a signature that stops being `const`, or
//! stops resolving in that position, fails to compile the fixture. The
//! compile-fail fixture pins the other half of the same boundary, that a
//! `JitterSample` is only ever built through its validating constructor.
//!
//! Gated behind the `trybuild-tests` feature so ordinary `cargo test` runs stay
//! fast; `make test` and CI enable it via `--all-features`.

#[cfg(feature = "trybuild-tests")]
#[test]
fn retry_const_api_keeps_its_compile_time_surface() {
    let t = trybuild::TestCases::new();
    t.pass("tests/support/ui/retry_const_api_const_context.rs");
    t.compile_fail("tests/support/ui/jitter_sample_private_constructor.rs");
}
