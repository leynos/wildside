//! Prometheus adapter for the domain pagination-error metrics port.
//!
//! Implements [`PaginationErrorMetrics`] with an `IntCounterVec` registered
//! against a caller-supplied [`prometheus::Registry`], keeping metric state off
//! process globals. The server composition root builds this adapter only when
//! the `metrics` feature is enabled and a Prometheus registry is configured,
//! and it is this counter that the `/metrics` endpoint exposes.

use prometheus::{IntCounterVec, Opts, Registry};

use crate::domain::ports::{PaginationErrorMetrics, PaginationErrorSource};

const PAGINATION_ERRORS_TOTAL: &str = "wildside_pagination_errors_total";
const PAGINATION_ERRORS_HELP: &str = "Total pagination cursor errors mapped to client responses";

/// Prometheus-backed recorder for pagination errors.
#[derive(Clone)]
pub struct PrometheusPaginationErrorMetrics {
    counter: IntCounterVec,
}

impl PrometheusPaginationErrorMetrics {
    /// Create and register the pagination-error counter with a registry.
    ///
    /// # Errors
    ///
    /// Returns an error if Prometheus rejects the metric or its registration.
    ///
    /// # Examples
    /// ```
    /// use backend::outbound::metrics::PrometheusPaginationErrorMetrics;
    /// use prometheus::Registry;
    ///
    /// let registry = Registry::new();
    /// let metrics = PrometheusPaginationErrorMetrics::new(&registry)?;
    /// # Ok::<(), prometheus::Error>(())
    /// ```
    pub fn new(registry: &Registry) -> Result<Self, prometheus::Error> {
        let counter = IntCounterVec::new(
            Opts::new(PAGINATION_ERRORS_TOTAL, PAGINATION_ERRORS_HELP),
            &["code", "source"],
        )?;
        registry.register(Box::new(counter.clone()))?;

        Ok(Self { counter })
    }
}

impl PaginationErrorMetrics for PrometheusPaginationErrorMetrics {
    fn increment(&self, source: PaginationErrorSource, detail_code: &'static str) {
        self.counter
            .with_label_values(&[detail_code, source.as_str()])
            .inc();
    }
}

#[cfg(test)]
mod tests {
    //! Verify the pagination counter's metric name, labels, and increments.

    use crate::domain::ports::{PaginationErrorMetrics, PaginationErrorSource};
    use crate::outbound::metrics::PrometheusPaginationErrorMetrics;
    use prometheus::Registry;
    use rstest::rstest;

    const METRIC_NAME: &str = "wildside_pagination_errors_total";
    const METRIC_HELP: &str = "Total pagination cursor errors mapped to client responses";

    /// Assert the registry holds exactly one metric with the expected labels.
    fn assert_counter(
        registry: &Registry,
        detail_code: &str,
        source_label: &str,
        expected_value: f64,
    ) {
        let families = registry.gather();
        let family = families
            .iter()
            .find(|family| family.name() == METRIC_NAME)
            .expect("pagination counter is registered");

        assert_eq!(family.help(), METRIC_HELP);
        assert_eq!(family.get_metric().len(), 1);

        let metric = &family.get_metric()[0];
        let labels = metric.get_label();
        assert_eq!(labels.len(), 2);
        assert_eq!(labels[0].name(), "code");
        assert_eq!(labels[0].value(), detail_code);
        assert_eq!(labels[1].name(), "source");
        assert_eq!(labels[1].value(), source_label);
        assert_eq!(
            metric.counter.as_ref().map(|counter| counter.value()),
            Some(expected_value)
        );
    }

    /// Assert each source/code pair increments its own labelled counter.
    #[rstest]
    #[case::http_invalid_cursor(PaginationErrorSource::UsersHttp, "invalid_cursor", "users_http")]
    #[case::http_unsupported_direction(
        PaginationErrorSource::UsersHttp,
        "unsupported_direction",
        "users_http"
    )]
    #[case::persistence_invalid_cursor(
        PaginationErrorSource::UserPersistence,
        "invalid_cursor",
        "user_persistence"
    )]
    #[case::persistence_unsupported_direction(
        PaginationErrorSource::UserPersistence,
        "unsupported_direction",
        "user_persistence"
    )]
    #[test]
    fn records_each_source_and_detail_code(
        #[case] source: PaginationErrorSource,
        #[case] detail_code: &'static str,
        #[case] source_label: &str,
    ) {
        let registry = Registry::new();
        let metrics = PrometheusPaginationErrorMetrics::new(&registry)
            .expect("pagination metrics registration succeeds");

        metrics.increment(source, detail_code);

        assert_counter(&registry, detail_code, source_label, 1.0);
    }

    /// Assert repeat increments accumulate on the same labelled series.
    #[test]
    fn increments_the_same_source_and_detail_code_twice() {
        let registry = Registry::new();
        let metrics = PrometheusPaginationErrorMetrics::new(&registry)
            .expect("pagination metrics registration succeeds");

        metrics.increment(PaginationErrorSource::UsersHttp, "invalid_cursor");
        metrics.increment(PaginationErrorSource::UsersHttp, "invalid_cursor");

        assert_counter(&registry, "invalid_cursor", "users_http", 2.0);
    }
}
