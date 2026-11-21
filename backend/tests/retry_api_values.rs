//! Runtime coverage for the retry API's documented labels and midpoint value.
//!
//! Whether these methods are callable in a `const` item is a compile-time
//! contract covered by the trybuild fixture at
//! `tests/support/ui/retry_const_api_const_context.rs`. This target keeps the
//! values themselves pinned, so a method that stays `const` but starts
//! returning a different label still fails.

use backend::domain::jobs::retry::JitterSample;
use backend::domain::ports::{RouteDeadLetterOutcome, RouteQueueOutcome, RouteRetryOutcome};
use backend::outbound::queue::TerminalRetryReason;

#[test]
fn retry_api_keeps_its_documented_values() {
    assert_eq!(
        JitterSample::midpoint(),
        JitterSample::new(0.5).expect("0.5 is a valid sample"),
        "the midpoint sample is the neutral 0.5 jitter"
    );
    assert_eq!(
        TerminalRetryReason::Exhausted.as_label(),
        "exhausted",
        "an exhausted retry reports the exhausted label"
    );
    assert_eq!(
        TerminalRetryReason::Fatal.as_label(),
        "fatal",
        "a fatal failure reports the fatal label"
    );
    assert_eq!(
        RouteQueueOutcome::Success.as_label(),
        "success",
        "a delivered enqueue reports the success label"
    );
    assert_eq!(
        RouteRetryOutcome::Scheduled.as_label(),
        "scheduled",
        "a scheduled retry reports the scheduled label"
    );
    assert_eq!(
        RouteDeadLetterOutcome::Exhausted.as_label(),
        "exhausted",
        "a dead-lettered exhaustion reports the exhausted label"
    );
}
