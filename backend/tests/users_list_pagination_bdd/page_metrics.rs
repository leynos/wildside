//! In-memory pagination telemetry for users-list BDD scenarios.

use std::sync::Mutex;

use backend::domain::ports::{
    LimitSource, PageOutcome, PageTraversal, PaginationPageMetrics, PaginationPageObservation,
};

use super::flow_support::World;

pub(crate) type PageMetricExpectation = (PageTraversal, PageOutcome, LimitSource, usize, usize);

#[derive(Default)]
pub(crate) struct RecordingPageMetrics(Mutex<Vec<PaginationPageObservation>>);

impl PaginationPageMetrics for RecordingPageMetrics {
    fn increment(&self, observation: PaginationPageObservation) {
        self.0
            .lock()
            .expect("page metrics lock should not be poisoned")
            .push(observation);
    }
}

impl RecordingPageMetrics {
    fn observations(&self) -> Vec<PaginationPageObservation> {
        self.0
            .lock()
            .expect("page metrics lock should not be poisoned")
            .clone()
    }
}

pub(crate) fn assert_page_metrics(world: &World, expected: &[PageMetricExpectation]) {
    let actual = world
        .page_metrics
        .observations()
        .into_iter()
        .map(|observation| {
            (
                observation.traversal,
                observation.outcome,
                observation.limit_source,
                observation.page_limit.get(),
                observation.returned_rows,
            )
        })
        .collect::<Vec<_>>();

    assert_eq!(
        actual, expected,
        "each successful request must record the expected traversal, outcome, limit source, limit, and row count"
    );
}
