//! Runtime coverage for the retry API's documented midpoint value.
//!
//! Whether the midpoint constructor is callable in a `const` item is a
//! compile-time contract covered by the trybuild fixture at
//! `tests/support/ui/retry_const_api_const_context.rs`. This target keeps the
//! value itself pinned, so a method that stays `const` but starts returning a
//! different midpoint still fails.
//!
//! The route queue outcome labels are no longer part of the domain API; they
//! moved to the metrics adapter, which pins their strings in its own tests.

use backend::domain::jobs::retry::JitterSample;

#[test]
fn retry_api_keeps_its_documented_midpoint() {
    assert_eq!(
        JitterSample::midpoint(),
        JitterSample::new(0.5).expect("0.5 is a valid sample"),
        "the midpoint sample is the neutral 0.5 jitter"
    );
}
