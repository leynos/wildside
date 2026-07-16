//! Compile-fail coverage for the `declare_test_support!` module-wiring macro.
//!
//! The macro's happy path is exercised at compile time by every BDD and
//! integration binary that invokes `declare_test_support!(...)` and then uses
//! the re-exported `support::*` helpers. This test locks in the negative path:
//! an unregistered `@module` name must fail during macro expansion with a clear
//! diagnostic rather than expanding to nothing.
//!
//! Gated behind the `trybuild-tests` feature so ordinary `cargo test` runs stay
//! fast; `make test` and CI enable it via `--all-features`.

#[cfg(feature = "trybuild-tests")]
#[test]
fn declare_test_support_rejects_unknown_module() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/support/ui/declare_test_support_unknown_module.rs");
}
