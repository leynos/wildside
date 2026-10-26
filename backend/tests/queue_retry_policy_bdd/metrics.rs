//! Recording queue metrics used by retry-policy BDD scenarios.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use backend::domain::ports::{
    RouteDeadLetterOutcome, RouteQueueMetrics, RouteQueueOutcome, RouteRetryOutcome,
};

#[derive(Clone, Default)]
pub struct RecordingRetryMetrics {
    pub retry_outcomes: Arc<Mutex<Vec<RouteRetryOutcome>>>,
    pub dead_letter_outcomes: Arc<Mutex<Vec<RouteDeadLetterOutcome>>>,
}

impl RouteQueueMetrics for RecordingRetryMetrics {
    fn observe_enqueue(&self, _outcome: RouteQueueOutcome, _latency: Duration) {}

    fn observe_retry(&self, outcome: RouteRetryOutcome) {
        self.retry_outcomes
            .lock()
            .expect("retry metrics mutex")
            .push(outcome);
    }

    fn observe_dead_letter(&self, outcome: RouteDeadLetterOutcome) {
        self.dead_letter_outcomes
            .lock()
            .expect("dead-letter metrics mutex")
            .push(outcome);
    }
}
