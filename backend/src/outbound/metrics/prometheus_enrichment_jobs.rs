//! Prometheus adapter for Overpass enrichment job counters.
//!
//! This adapter writes two counter families so dashboards can query either a
//! shared multi-job metric (`jobs_total`) or an enrichment-focused metric
//! (`enrichment_jobs_total`) without domain coupling.

use async_trait::async_trait;
use prometheus::{CounterVec, Opts, Registry};

use crate::domain::ports::{
    EnrichmentJobFailure, EnrichmentJobMetrics, EnrichmentJobMetricsError, EnrichmentJobSuccess,
};

const ENRICHMENT_TYPE_LABEL: &str = "Enrichment";

/// Prometheus-backed recorder for enrichment job outcomes.
pub struct PrometheusEnrichmentJobMetrics {
    jobs_total: CounterVec,
    enrichment_jobs_total: CounterVec,
}

impl PrometheusEnrichmentJobMetrics {
    /// Create and register counters with the provided registry.
    ///
    /// # Errors
    ///
    /// Returns an error when Prometheus rejects metric registration.
    pub fn new(registry: &Registry) -> Result<Self, prometheus::Error> {
        let jobs_total = CounterVec::new(
            Opts::new("jobs_total", "Total jobs by type and status"),
            &["type", "status"],
        )?;
        let enrichment_jobs_total = CounterVec::new(
            Opts::new("enrichment_jobs_total", "Total enrichment jobs by status"),
            &["status"],
        )?;
        registry.register(Box::new(jobs_total.clone()))?;
        registry.register(Box::new(enrichment_jobs_total.clone()))?;
        Ok(Self {
            jobs_total,
            enrichment_jobs_total,
        })
    }

    fn record(&self, status: &str) {
        self.jobs_total
            .with_label_values(&[ENRICHMENT_TYPE_LABEL, status])
            .inc();
        self.enrichment_jobs_total
            .with_label_values(&[status])
            .inc();
    }
}

#[async_trait]
impl EnrichmentJobMetrics for PrometheusEnrichmentJobMetrics {
    async fn record_success(
        &self,
        _payload: &EnrichmentJobSuccess,
    ) -> Result<(), EnrichmentJobMetricsError> {
        self.record("success");
        Ok(())
    }

    async fn record_failure(
        &self,
        _payload: &EnrichmentJobFailure,
    ) -> Result<(), EnrichmentJobMetricsError> {
        self.record("failure");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    //! Regression coverage for enrichment job counters.

    use super::*;
    use crate::domain::ports::{EnrichmentJobFailureKind, EnrichmentJobSuccess};
    use rstest::rstest;

    fn make_metrics() -> (Registry, PrometheusEnrichmentJobMetrics) {
        let registry = Registry::new();
        let metrics = PrometheusEnrichmentJobMetrics::new(&registry)
            .expect("metric registration should succeed");
        (registry, metrics)
    }

    #[test]
    fn registers_counters_with_registry() {
        let (registry, metrics) = make_metrics();
        metrics.record("success");
        let families = registry.gather();

        assert!(
            families.iter().any(|metric| metric.name() == "jobs_total"),
            "jobs_total should be registered"
        );
        assert!(
            families
                .iter()
                .any(|metric| metric.name() == "enrichment_jobs_total"),
            "enrichment_jobs_total should be registered"
        );
    }

    #[rstest]
    #[case::success("success")]
    #[case::failure("failure")]
    #[tokio::test]
    async fn records_outcome_in_both_metric_families(#[case] status: &str) {
        let (_registry, metrics) = make_metrics();

        match status {
            "success" => {
                metrics
                    .record_success(&EnrichmentJobSuccess {
                        attempt_count: 1,
                        persisted_poi_count: 4,
                        transfer_bytes: 1_024,
                    })
                    .await
                    .expect("recording success should not fail");
            }
            "failure" => {
                metrics
                    .record_failure(&crate::domain::ports::EnrichmentJobFailure {
                        attempt_count: 2,
                        kind: EnrichmentJobFailureKind::RetryExhausted,
                    })
                    .await
                    .expect("recording failure should not fail");
            }
            _ => panic!("unknown status case: {status}"),
        }

        let jobs_total = metrics
            .jobs_total
            .with_label_values(&[ENRICHMENT_TYPE_LABEL, status]);
        let enrichment_total = metrics.enrichment_jobs_total.with_label_values(&[status]);

        assert_eq!(
            jobs_total.get() as u64,
            1,
            "jobs_total should increment for {status}",
        );
        assert_eq!(
            enrichment_total.get() as u64,
            1,
            "enrichment_jobs_total should increment for {status}",
        );
    }
}
