//! Compile-fail fixture: a jitter sample cannot bypass its validating
//! constructor.
//!
//! `JitterSample` keeps its field private so every sample in the system came
//! through `JitterSample::new`, which enforces the `[0, 1]` range that
//! `JobRetryPolicy::backoff_delay` relies on. If this fixture compiles, that
//! invariant has been lost and an out-of-range jitter sample could be stored.

use backend::domain::jobs::retry::JitterSample;

fn main() {
    let _ = JitterSample(1.5);
}