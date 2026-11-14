//! Compile-time coverage for the retry API's public `const fn`s.
//!
//! Each item below is evaluated by the compiler, not at run time. If a
//! function loses its `const` qualifier, or its signature stops being usable in
//! a constant context, this target fails to compile. That is the whole
//! assertion: the test body only re-checks the values so a reader can see what
//! the constants are expected to hold.

use backend::domain::jobs::retry::JitterSample;
use backend::domain::ports::{RouteDeadLetterOutcome, RouteQueueOutcome, RouteRetryOutcome};
use backend::outbound::queue::TerminalRetryReason;

/// `JitterSample::midpoint` must be callable in a constant context.
const MIDPOINT: JitterSample = JitterSample::midpoint();

/// `TerminalRetryReason::as_label` must be callable in a constant context.
const EXHAUSTED_LABEL: &str = TerminalRetryReason::Exhausted.as_label();
const FATAL_LABEL: &str = TerminalRetryReason::Fatal.as_label();

/// The metric label methods must be callable in a constant context.
const ENQUEUE_LABEL: &str = RouteQueueOutcome::Success.as_label();
const RETRY_LABEL: &str = RouteRetryOutcome::Scheduled.as_label();
const DEAD_LETTER_LABEL: &str = RouteDeadLetterOutcome::Exhausted.as_label();

#[test]
fn const_retry_api_keeps_its_documented_values() {
    assert_eq!(
        MIDPOINT,
        JitterSample::new(0.5).expect("0.5 is a valid sample")
    );
    assert_eq!(EXHAUSTED_LABEL, "exhausted");
    assert_eq!(FATAL_LABEL, "fatal");
    assert_eq!(ENQUEUE_LABEL, "success");
    assert_eq!(RETRY_LABEL, "scheduled");
    assert_eq!(DEAD_LETTER_LABEL, "exhausted");
}
