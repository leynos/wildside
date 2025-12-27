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
    use super::*;

    #[test]
    fn registers_metric_with_registry() {
        let registry = Registry::new();
        let metrics = PrometheusIdempotencyMetrics::new(&registry)
            .expect("metric registration should succeed");

        let labels = IdempotencyMetricLabels {
            user_scope: "a1b2c3d4".to_string(),
            age_bucket: None,
        };
        metrics.record("miss", &labels);

        // Verify the metric was registered by checking the registry.
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
        let registry = Registry::new();
        let metrics = PrometheusIdempotencyMetrics::new(&registry)
            .expect("metric registration should succeed");

        let labels = IdempotencyMetricLabels {
            user_scope: "deadbeef".to_string(),
            age_bucket: Some("1-5m".to_string()),
        };
        metrics.record("hit", &labels);
        metrics.record("hit", &labels);

        // Get the counter value directly.
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
        let registry = Registry::new();
        let metrics = PrometheusIdempotencyMetrics::new(&registry)
            .expect("metric registration should succeed");

        let labels = IdempotencyMetricLabels {
            user_scope: "abcd1234".to_string(),
            age_bucket: None,
        };
        metrics.record("miss", &labels);

        // Verify the n/a label is used.
        let counter = metrics
            .requests_total
            .with_label_values(&["miss", "abcd1234", "n/a"]);
        assert_eq!(
            counter.get() as u64,
            1,
            "counter should use n/a for missing age bucket"
        );
    }

    #[tokio::test]
    async fn record_miss_increments_counter() {
        let registry = Registry::new();
        let metrics = PrometheusIdempotencyMetrics::new(&registry)
            .expect("metric registration should succeed");

        let labels = IdempotencyMetricLabels {
            user_scope: "test1234".to_string(),
            age_bucket: None,
        };
        metrics
            .record_miss(&labels)
            .await
            .expect("recording should succeed");

        let counter = metrics
            .requests_total
            .with_label_values(&["miss", "test1234", "n/a"]);
        assert_eq!(counter.get() as u64, 1);
    }

    #[tokio::test]
    async fn record_hit_increments_counter() {
        let registry = Registry::new();
        let metrics = PrometheusIdempotencyMetrics::new(&registry)
            .expect("metric registration should succeed");

        let labels = IdempotencyMetricLabels {
            user_scope: "test5678".to_string(),
            age_bucket: Some("5-30m".to_string()),
        };
        metrics
            .record_hit(&labels)
            .await
            .expect("recording should succeed");

        let counter = metrics
            .requests_total
            .with_label_values(&["hit", "test5678", "5-30m"]);
        assert_eq!(counter.get() as u64, 1);
    }

    #[tokio::test]
    async fn record_conflict_increments_counter() {
        let registry = Registry::new();
        let metrics = PrometheusIdempotencyMetrics::new(&registry)
            .expect("metric registration should succeed");

        let labels = IdempotencyMetricLabels {
            user_scope: "conflictu".to_string(),
            age_bucket: Some("30m-2h".to_string()),
        };
        metrics
            .record_conflict(&labels)
            .await
            .expect("recording should succeed");

        let counter =
            metrics
                .requests_total
                .with_label_values(&["conflict", "conflictu", "30m-2h"]);
        assert_eq!(counter.get() as u64, 1);
    }
}
