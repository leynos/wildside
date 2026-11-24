//! Prometheus adapter for successful pagination page-delivery metrics.
//!
//! The adapter registers two bounded counter families with an injected
//! registry and resolves every label child during construction. Recording a
//! page therefore performs only the two atomic counter increments.

use prometheus::{IntCounter, IntCounterVec, Opts, Registry};

use crate::domain::ports::{
    LimitSource, PageOutcome, PageTraversal, PaginatedEndpoint, PaginationPageMetrics,
    PaginationPageObservation,
};

const PAGES_TOTAL_NAME: &str = "wildside_pagination_pages_total";
const PAGES_TOTAL_HELP: &str = "Total pagination pages returned by endpoint and traversal";
const ROWS_RETURNED_TOTAL_NAME: &str = "wildside_pagination_rows_returned_total";
const ROWS_RETURNED_TOTAL_HELP: &str = "Total rows returned by endpoint and traversal";

const PAGE_LABEL_NAMES: &[&str] = &["endpoint", "traversal", "outcome", "limit_source"];
const ROW_LABEL_NAMES: &[&str] = &["endpoint", "traversal"];

const TRAVERSALS: [PageTraversal; 3] = [
    PageTraversal::First,
    PageTraversal::Next,
    PageTraversal::Prev,
];
const OUTCOMES: [PageOutcome; 2] = [PageOutcome::Continuing, PageOutcome::Terminal];
const LIMIT_SOURCES: [LimitSource; 3] = [
    LimitSource::Default,
    LimitSource::Explicit,
    LimitSource::Clamped,
];

const PAGE_SERIES_COUNT: usize = TRAVERSALS.len() * OUTCOMES.len() * LIMIT_SOURCES.len();
const ROW_SERIES_COUNT: usize = TRAVERSALS.len();

/// Prometheus-backed recorder for successful pagination page deliveries.
#[derive(Clone)]
pub struct PrometheusPaginationPageMetrics {
    pages_total: [IntCounter; PAGE_SERIES_COUNT],
    rows_returned_total: [IntCounter; ROW_SERIES_COUNT],
}

impl PrometheusPaginationPageMetrics {
    /// Register the page and row counters and resolve their bounded label sets.
    ///
    /// # Errors
    ///
    /// Returns an error if Prometheus rejects metric creation or registration.
    ///
    /// # Examples
    /// ```
    /// use backend::outbound::metrics::PrometheusPaginationPageMetrics;
    /// use prometheus::Registry;
    ///
    /// let registry = Registry::new();
    /// let metrics = PrometheusPaginationPageMetrics::new(&registry)?;
    /// # Ok::<(), prometheus::Error>(())
    /// ```
    pub fn new(registry: &Registry) -> Result<Self, prometheus::Error> {
        let pages = IntCounterVec::new(
            Opts::new(PAGES_TOTAL_NAME, PAGES_TOTAL_HELP),
            PAGE_LABEL_NAMES,
        )?;
        let rows = IntCounterVec::new(
            Opts::new(ROWS_RETURNED_TOTAL_NAME, ROWS_RETURNED_TOTAL_HELP),
            ROW_LABEL_NAMES,
        )?;

        registry.register(Box::new(pages.clone()))?;
        registry.register(Box::new(rows.clone()))?;

        let pages_total = std::array::from_fn(|index| {
            let traversal = TRAVERSALS[index / (OUTCOMES.len() * LIMIT_SOURCES.len())];
            let outcome = OUTCOMES[(index / LIMIT_SOURCES.len()) % OUTCOMES.len()];
            let limit_source = LIMIT_SOURCES[index % LIMIT_SOURCES.len()];
            let labels =
                page_label_values(PaginatedEndpoint::Users, traversal, outcome, limit_source);

            pages.with_label_values(&labels)
        });
        let rows_returned_total = std::array::from_fn(|index| {
            let labels = row_label_values(PaginatedEndpoint::Users, TRAVERSALS[index]);
            rows.with_label_values(&labels)
        });

        Ok(Self {
            pages_total,
            rows_returned_total,
        })
    }
}

impl PaginationPageMetrics for PrometheusPaginationPageMetrics {
    fn increment(&self, observation: PaginationPageObservation) {
        let page_index = page_series_index(observation);
        let traversal_index = traversal_index(observation.traversal);
        let returned_rows = observation.returned_rows.min(observation.page_limit.get());

        self.pages_total[page_index].inc();
        self.rows_returned_total[traversal_index]
            .inc_by(u64::try_from(returned_rows).unwrap_or(u64::MAX));
    }
}

const fn page_series_index(observation: PaginationPageObservation) -> usize {
    traversal_index(observation.traversal) * OUTCOMES.len() * LIMIT_SOURCES.len()
        + outcome_index(observation.outcome) * LIMIT_SOURCES.len()
        + limit_source_index(observation.limit_source)
}

const fn traversal_index(traversal: PageTraversal) -> usize {
    match traversal {
        PageTraversal::First => 0,
        PageTraversal::Next => 1,
        PageTraversal::Prev => 2,
    }
}

const fn outcome_index(outcome: PageOutcome) -> usize {
    match outcome {
        PageOutcome::Continuing => 0,
        PageOutcome::Terminal => 1,
    }
}

const fn limit_source_index(limit_source: LimitSource) -> usize {
    match limit_source {
        LimitSource::Default => 0,
        LimitSource::Explicit => 1,
        LimitSource::Clamped => 2,
    }
}

const fn page_label_values(
    endpoint: PaginatedEndpoint,
    traversal: PageTraversal,
    outcome: PageOutcome,
    limit_source: LimitSource,
) -> [&'static str; 4] {
    [
        endpoint.as_label(),
        traversal.as_label(),
        outcome.as_label(),
        limit_source.as_label(),
    ]
}

const fn row_label_values(
    endpoint: PaginatedEndpoint,
    traversal: PageTraversal,
) -> [&'static str; 2] {
    [endpoint.as_label(), traversal.as_label()]
}
