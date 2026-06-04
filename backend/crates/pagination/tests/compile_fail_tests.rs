//! Trybuild compile-fail tests for cursor trait bounds.
//!
//! Gated behind the `trybuild-tests` feature so local `cargo test` runs stay
//! fast. CI activates the feature in a dedicated step that calls
//! `cargo test --features trybuild-tests -p pagination
//! cursor_trait_bound_compile_fail_tests`.

#[cfg(feature = "trybuild-tests")]
#[test]
fn cursor_trait_bound_compile_fail_tests() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/cursor_encode_no_serialize.rs");
    t.compile_fail("tests/ui/cursor_decode_no_deserialise_owned.rs");
}
