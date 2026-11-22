//! Compile-pass fixture: the retry API's `const fn`s stay usable in a
//! constant context.
//!
//! Every item below is evaluated by the compiler rather than at run time. If
//! `JitterSample::midpoint` or an outcome-label method loses its `const`
//! qualifier, or its signature stops resolving here, trybuild reports the
//! fixture as a failed compilation. The constants are read in `main` so the
//! fixture itself compiles without unused-item warnings.

use backend::domain::jobs::retry::JitterSample;
use backend::domain::ports::{RouteDeadLetterOutcome, RouteQueueOutcome, RouteRetryOutcome};

const MIDPOINT: JitterSample = JitterSample::midpoint();
const ENQUEUE_LABEL: &str = RouteQueueOutcome::Success.as_label();
const RETRY_LABEL: &str = RouteRetryOutcome::Scheduled.as_label();
const DEAD_LETTER_LABEL: &str = RouteDeadLetterOutcome::Exhausted.as_label();

fn main() {
    let _ = (
        MIDPOINT,
        ENQUEUE_LABEL,
        RETRY_LABEL,
        DEAD_LETTER_LABEL,
    );
}