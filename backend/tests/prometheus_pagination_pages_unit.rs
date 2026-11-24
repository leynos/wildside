//! Behavioural contracts for the Prometheus pagination page adapter.

#![cfg(feature = "metrics")]

use std::num::NonZeroUsize;

use backend::domain::ports::{
    LimitSource, PageOutcome, PageTraversal, PaginatedEndpoint, PaginationPageMetrics,
    PaginationPageObservation,
};
use backend::outbound::metrics::PrometheusPaginationPageMetrics;
use prometheus::{Registry, proto::Metric, proto::MetricFamily};

const PAGES_TOTAL: &str = "wildside_pagination_pages_total";
const ROWS_RETURNED_TOTAL: &str = "wildside_pagination_rows_returned_total";

fn find_family<'a>(families: &'a [MetricFamily], name: &str) -> &'a MetricFamily {
    families
        .iter()
        .find(|family| family.name() == name)
        .expect("the pagination metric family is registered")
}

fn find_metric<'a>(family: &'a MetricFamily, expected_labels: &[(&str, &str)]) -> &'a Metric {
    family
        .get_metric()
        .iter()
        .find(|metric| {
            expected_labels
                .iter()
                .all(|(expected_name, expected_value)| {
                    metric.get_label().iter().any(|pair| {
                        pair.name() == *expected_name && pair.value() == *expected_value
                    })
                })
        })
        .expect("the expected label name/value pairing is gathered")
}

#[test]
fn every_supported_label_combination_gathers_as_a_distinct_series() {
    let registry = Registry::new();
    let metrics = PrometheusPaginationPageMetrics::new(&registry)
        .expect("pagination page metrics register with a fresh registry");

    for traversal in [
        PageTraversal::First,
        PageTraversal::Next,
        PageTraversal::Prev,
    ] {
        let returned_rows = match traversal {
            PageTraversal::First => 1,
            PageTraversal::Next => 2,
            PageTraversal::Prev => 3,
        };
        for outcome in [PageOutcome::Continuing, PageOutcome::Terminal] {
            for limit_source in [
                LimitSource::Default,
                LimitSource::Explicit,
                LimitSource::Clamped,
            ] {
                metrics.increment(PaginationPageObservation {
                    endpoint: PaginatedEndpoint::Users,
                    traversal,
                    outcome,
                    limit_source,
                    page_limit: NonZeroUsize::new(20).expect("test page limit is non-zero"),
                    returned_rows,
                });
            }
        }
    }

    let families = registry.gather();
    let pages = find_family(&families, PAGES_TOTAL);
    let rows = find_family(&families, ROWS_RETURNED_TOTAL);

    assert_eq!(
        pages.get_metric().len() + rows.get_metric().len(),
        21,
        "the adapter must expose exactly 21 bounded series per endpoint"
    );
    assert_eq!(
        pages.get_metric().len(),
        18,
        "the page counter must expose all 3 x 2 x 3 bounded label combinations"
    );
    assert_eq!(
        rows.get_metric().len(),
        3,
        "the rows counter must expose one series for each traversal"
    );
    assert!(
        pages
            .get_metric()
            .iter()
            .all(|metric| metric.get_counter().value() == 1.0),
        "each page label combination must receive exactly one increment"
    );
    for (traversal, expected_rows) in [
        (PageTraversal::First, 6.0),
        (PageTraversal::Next, 12.0),
        (PageTraversal::Prev, 18.0),
    ] {
        let row = find_metric(
            rows,
            &[("endpoint", "users"), ("traversal", traversal.as_label())],
        );
        assert_eq!(
            row.get_counter().value(),
            expected_rows,
            "the row total must match the expected traversal label"
        );
    }
}

#[test]
fn gathered_series_pair_label_names_with_their_intended_values() {
    let registry = Registry::new();
    let metrics = PrometheusPaginationPageMetrics::new(&registry)
        .expect("pagination page metrics register with a fresh registry");
    metrics.increment(PaginationPageObservation {
        endpoint: PaginatedEndpoint::Users,
        traversal: PageTraversal::Prev,
        outcome: PageOutcome::Terminal,
        limit_source: LimitSource::Clamped,
        page_limit: NonZeroUsize::new(20).expect("test page limit is non-zero"),
        returned_rows: 4,
    });

    let families = registry.gather();
    let pages = find_family(&families, PAGES_TOTAL);
    let page = find_metric(
        pages,
        &[
            ("endpoint", "users"),
            ("traversal", "prev"),
            ("outcome", "terminal"),
            ("limit_source", "clamped"),
        ],
    );

    assert_eq!(
        page.get_counter().value(),
        1.0,
        "the intended page series must receive the increment"
    );

    let rows = find_family(&families, ROWS_RETURNED_TOTAL);
    let row = find_metric(rows, &[("endpoint", "users"), ("traversal", "prev")]);
    assert_eq!(
        row.get_counter().value(),
        4.0,
        "the rows counter must add the number of returned rows"
    );
}
