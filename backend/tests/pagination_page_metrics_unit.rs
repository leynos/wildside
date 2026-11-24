//! Behavioural contracts for the pagination page metrics domain port.

use std::num::NonZeroUsize;

use backend::domain::ports::{
    LimitSource, NoOpPaginationPageMetrics, PageOutcome, PageTraversal, PaginatedEndpoint,
    PaginationPageDelivery, PaginationPageMetrics, PaginationPageObservation,
};
use proptest::prelude::*;
use rstest::rstest;
use tracing_test::traced_test;

/// A no-op adapter must retain the structured event in metrics-free builds.
#[traced_test]
#[rstest]
#[case::first(PageTraversal::First, PageOutcome::Terminal, LimitSource::Default)]
#[case::next(PageTraversal::Next, PageOutcome::Continuing, LimitSource::Explicit)]
#[case::prev(PageTraversal::Prev, PageOutcome::Terminal, LimitSource::Clamped)]
fn no_op_record_emits_page_fields(
    #[case] traversal: PageTraversal,
    #[case] outcome: PageOutcome,
    #[case] limit_source: LimitSource,
) {
    let observation = PaginationPageObservation {
        endpoint: PaginatedEndpoint::Users,
        traversal,
        outcome,
        limit_source,
        page_limit: NonZeroUsize::new(5).expect("test page limit is non-zero"),
        returned_rows: 3,
    };

    NoOpPaginationPageMetrics.record(observation);

    assert!(
        logs_contain("endpoint=\"users\""),
        "the page event must identify the users endpoint"
    );
    assert!(
        logs_contain(&format!("traversal=\"{}\"", traversal.as_label())),
        "the page event must include its traversal label"
    );
    assert!(
        logs_contain(&format!("outcome=\"{}\"", outcome.as_label())),
        "the page event must include its outcome label"
    );
    assert!(
        logs_contain(&format!("limit_source=\"{}\"", limit_source.as_label())),
        "the page event must include its limit-source label"
    );
    assert!(
        logs_contain("page_limit=5"),
        "the page event must include the effective page limit"
    );
    assert!(
        logs_contain("rows=3"),
        "the page event must include the returned row count"
    );
    assert!(
        logs_contain("trace_id=\"unknown\""),
        "the page event must include the trace identifier field"
    );
}

// Saturation must preserve row and continuation invariants for all inputs.
proptest! {
    #[test]
    fn observation_saturates_rows_and_short_pages(
        page_limit in 1usize..=100,
        rows_below_limit in 0usize..=100,
        rows_above_limit in 101usize..=usize::MAX,
        has_more in any::<bool>(),
    ) {
        let page_limit = NonZeroUsize::new(page_limit)
            .expect("generated page limit is non-zero");
        let rows_below_limit = rows_below_limit.min(page_limit.get() - 1);
        let rows_at_limit = page_limit.get();
        let rows_above_limit = rows_above_limit.max(page_limit.get() + 1);

        prop_assert!(
            rows_below_limit < page_limit.get(),
            "the short-page generator must reach a count below the limit"
        );
        prop_assert_eq!(
            rows_at_limit,
            page_limit.get(),
            "the full-page generator must reach the page limit"
        );
        prop_assert!(
            rows_above_limit > page_limit.get(),
            "the overflow generator must reach an out-of-range row count"
        );

        for (returned_rows, has_more) in [
            (rows_below_limit, has_more),
            (rows_below_limit, !has_more),
            (rows_at_limit, has_more),
            (rows_at_limit, !has_more),
            (rows_above_limit, has_more),
            (rows_above_limit, !has_more),
        ] {
            let observation = PaginationPageObservation::new(
                PaginatedEndpoint::Users,
                PageTraversal::Next,
                PaginationPageDelivery {
                    limit_source: LimitSource::Explicit,
                    page_limit,
                    returned_rows,
                    has_more,
                },
            );

            prop_assert!(
                observation.returned_rows <= observation.page_limit.get(),
                "returned rows must saturate at the page limit"
            );
            if observation.returned_rows < observation.page_limit.get() {
                prop_assert_eq!(
                    observation.outcome,
                    PageOutcome::Terminal,
                    "a short page cannot claim another page"
                );
            }
        }
    }
}
