//! Prometheus adapter for idempotency audit metrics.
//!
//! This adapter exports idempotency request outcomes to Prometheus via the
//! `prometheus` crate. Metrics are registered with a provided registry and
//! exposed via the `/metrics` endpoint.

use async_trait::async_trait;
use prometheus::{CounterVec, Opts, Registry};

use crate::domain::ports::{IdempotencyMetricLabels, IdempotencyMetrics, IdempotencyMetricsError};

/// Prometheus-backed idempotency metrics recorder.
///
/// Records hits, misses, and conflicts as increments to a single counter
/// metric with labels for outcome, user scope, and age bucket.
///
/// # Metric Specification
///
/// - **Name**: `wildside_idempotency_requests_total`
/// - **Type**: Counter
/// - **Labels**:
///   - `outcome`: `miss`, `hit`, or `conflict`
///   - `user_scope`: 8-character hex hash of user ID
///   - `age_bucket`: `0-1m`, `1-5m`, `5-30m`, `30m-2h`, `2h-6h`, `6h-24h`, or `n/a`
pub struct PrometheusIdempotencyMetrics {
    requests_total: CounterVec,
}

impl PrometheusIdempotencyMetrics {
    /// Create and register metrics with the given registry.
    ///
    /// # Errors
    ///
    /// Returns an error if the metric cannot be registered (e.g., if a metric
    /// with the same name already exists in the registry).
    pub fn new(registry: &Registry) -> Result<Self, prometheus::Error> {
        let requests_total = CounterVec::new(
            Opts::new(
                "wildside_idempotency_requests_total",
                "Total idempotency requests by outcome",
            ),
            &["outcome", "user_scope", "age_bucket"],
        )?;
        registry.register(Box::new(requests_total.clone()))?;
        Ok(Self { requests_total })
    }

    /// Record a metric with the given outcome and labels.
    fn record(&self, outcome: &str, labels: &IdempotencyMetricLabels) {
        let age_bucket = labels.age_bucket.as_deref().unwrap_or("n/a");
        self.requests_total
            .with_label_values(&[outcome, &labels.user_scope, age_bucket])
            .inc();
    }
}

#[async_trait]
impl IdempotencyMetrics for PrometheusIdempotencyMetrics {
    async fn record_miss(
        &self,
        labels: &IdempotencyMetricLabels,
    ) -> Result<(), IdempotencyMetricsError> {
        self.record("miss", labels);
        Ok(())
    }

    async fn record_hit(
        &self,
        labels: &IdempotencyMetricLabels,
    ) -> Result<(), IdempotencyMetricsError> {
        self.record("hit", labels);
        Ok(())
    }

    async fn record_conflict(
        &self,
        labels: &IdempotencyMetricLabels,
    ) -> Result<(), IdempotencyMetricsError> {
        self.record("conflict", labels);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    //! Regression coverage for this module.
    use super::*;
    use rstest::rstest;

    /// Helper to create a fresh metrics instance for testing.
    fn make_metrics() -> (Registry, PrometheusIdempotencyMetrics) {
        let registry = Registry::new();
        let metrics = PrometheusIdempotencyMetrics::new(&registry)
            .expect("metric registration should succeed");
        (registry, metrics)
    }

    /// Helper to record a metric by outcome type and verify the counter.
    async fn record_and_verify(
        metrics: &PrometheusIdempotencyMetrics,
        outcome: &str,
        user_scope: &str,
        age_bucket: Option<&str>,
    ) {
        let labels = IdempotencyMetricLabels {
            user_scope: user_scope.to_string(),
            age_bucket: age_bucket.map(String::from),
        };

        let result = match outcome {
            "miss" => metrics.record_miss(&labels).await,
            "hit" => metrics.record_hit(&labels).await,
            "conflict" => metrics.record_conflict(&labels).await,
            _ => panic!("unknown outcome: {outcome}"),
        };
        result.expect("recording should succeed");

        let expected_bucket = age_bucket.unwrap_or("n/a");
        let counter =
            metrics
                .requests_total
                .with_label_values(&[outcome, user_scope, expected_bucket]);
        assert_eq!(
            counter.get() as u64,
            1,
            "{outcome} counter should be incremented"
        );
    }

    #[test]
    fn registers_metric_with_registry() {
        let (registry, metrics) = make_metrics();

        let labels = IdempotencyMetricLabels {
            user_scope: "a1b2c3d4".to_string(),
            age_bucket: None,
        };
        metrics.record("miss", &labels);

        let families = registry.gather();
        assert!(
            families
                .iter()
                .any(|f| f.name() == "wildside_idempotency_requests_total"),
            "metric should be registered"
        );
    }

    #[test]
    fn increments_counter_with_correct_labels() {
        let (_registry, metrics) = make_metrics();

        let labels = IdempotencyMetricLabels {
            user_scope: "deadbeef".to_string(),
            age_bucket: Some("1-5m".to_string()),
        };
        metrics.record("hit", &labels);
        metrics.record("hit", &labels);

        let counter = metrics
            .requests_total
            .with_label_values(&["hit", "deadbeef", "1-5m"]);
        assert_eq!(
            counter.get() as u64,
            2,
            "counter should be incremented twice"
        );
    }

    #[test]
    fn uses_na_for_missing_age_bucket() {
        let (_registry, metrics) = make_metrics();

        let labels = IdempotencyMetricLabels {
            user_scope: "abcd1234".to_string(),
            age_bucket: None,
        };
        metrics.record("miss", &labels);

        let counter = metrics
            .requests_total
            .with_label_values(&["miss", "abcd1234", "n/a"]);
        assert_eq!(
            counter.get() as u64,
            1,
            "counter should use n/a for missing age bucket"
        );
    }

    /// Parameterised test for all three outcome recording methods.
    #[rstest]
    #[case::miss("miss", None)]
    #[case::hit("hit", Some("5-30m"))]
    #[case::conflict("conflict", Some("30m-2h"))]
    #[tokio::test]
    async fn records_outcome_and_increments_counter(
        #[case] outcome: &str,
        #[case] age_bucket: Option<&str>,
    ) {
        let (_registry, metrics) = make_metrics();
        record_and_verify(&metrics, outcome, "testuser", age_bucket).await;
    }
}
