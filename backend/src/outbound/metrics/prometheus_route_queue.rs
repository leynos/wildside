//! Prometheus adapter for route queue enqueue metrics.
//!
//! This adapter records route queue enqueue operations when the `metrics`
//! Cargo feature is enabled. It registers `route_queue_enqueue_total`, an
//! `IntCounterVec` labelled by `outcome`, and
//! `route_queue_enqueue_latency_seconds`, a `HistogramVec` labelled by
//! `outcome`.
//!
//! The adapter receives a Prometheus [`Registry`] at construction time, so
//! tests can use isolated registries and production can use the process default
//! registry without coupling the queue adapter to global metrics state.

#![cfg(feature = "metrics")]

use std::time::Duration;

use prometheus::{HistogramOpts, HistogramVec, IntCounterVec, Opts, Registry};

use crate::domain::ports::{RouteQueueMetrics, RouteQueueOutcome};

// Total enqueue attempts, labelled only by bounded `outcome` values:
// "success" and "failure". No other labels are present, keeping cardinality
// low.
const QUEUE_ENQUEUE_TOTAL: &str = "route_queue_enqueue_total";
// Enqueue latency histogram, labelled only by bounded `outcome` values:
// "success" and "failure". No other labels are present, keeping cardinality
// low.
const QUEUE_ENQUEUE_LATENCY_SECONDS: &str = "route_queue_enqueue_latency_seconds";
// Fixed latency bucket ceilings, in seconds, used for route queue enqueue
// observations.
const ENQUEUE_LATENCY_BUCKETS_SECONDS: &[f64] = &[
    0.0005, 0.001, 0.0025, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0,
];

/// Prometheus-backed recorder for route queue enqueue outcomes.
#[derive(Clone)]
pub struct PrometheusRouteQueueMetrics {
    total: IntCounterVec,
    latency_seconds: HistogramVec,
}

impl PrometheusRouteQueueMetrics {
    /// Create and register route queue metrics with the provided registry.
    ///
    /// # Errors
    ///
    /// Returns an error when Prometheus rejects metric creation or
    /// registration.
    pub fn new(registry: &Registry) -> Result<Self, prometheus::Error> {
        let total = IntCounterVec::new(
            Opts::new(QUEUE_ENQUEUE_TOTAL, "Total route queue enqueue attempts"),
            &["outcome"],
        )?;

        let latency_seconds = HistogramVec::new(
            HistogramOpts::new(
                QUEUE_ENQUEUE_LATENCY_SECONDS,
                "Route queue enqueue latency by outcome in seconds",
            )
            .buckets(ENQUEUE_LATENCY_BUCKETS_SECONDS.to_vec()),
            &["outcome"],
        )?;

        registry.register(Box::new(total.clone()))?;
        registry.register(Box::new(latency_seconds.clone()))?;

        Ok(Self {
            total,
            latency_seconds,
        })
    }
}

impl RouteQueueMetrics for PrometheusRouteQueueMetrics {
    fn observe_enqueue(&self, outcome: RouteQueueOutcome, latency: Duration) {
        let outcome_label = outcome.as_label();
        self.total.with_label_values(&[outcome_label]).inc();
        self.latency_seconds
            .with_label_values(&[outcome_label])
            .observe(latency.as_secs_f64());
    }
}

#[cfg(test)]
mod tests {
    //! Tests for concurrent Prometheus route queue metric registration.

    use std::sync::Arc;

    use super::*;
    use insta::assert_snapshot;
    use prometheus::Encoder;
    use proptest::prelude::*;
    use tokio::task::JoinHandle;

    #[tokio::test]
    async fn initialization_is_concurrency_safe_per_registry() {
        let registry = Arc::new(Registry::new());
        let mut tasks = Vec::new();

        for _ in 0..8 {
            tasks.push(spawn_metrics_registration(Arc::clone(&registry)));
        }

        let mut successes = 0;
        for task in tasks {
            successes += handle_registration_result(task.await.expect("metrics task"));
        }

        assert_eq!(successes, 1, "one registration should win per registry");
    }

    #[test]
    fn records_success_and_failure_metrics_snapshot() {
        let registry = Registry::new();
        let metrics = new_metrics(&registry).expect("metrics register");

        metrics.observe_enqueue(RouteQueueOutcome::Success, Duration::from_millis(1));
        metrics.observe_enqueue(RouteQueueOutcome::Failure, Duration::from_millis(250));

        assert_snapshot!(
            encode_route_queue_metrics(&registry),
            @r###"
route_queue_enqueue_latency_seconds_bucket{outcome="failure",le="0.0005"} 0
route_queue_enqueue_latency_seconds_bucket{outcome="failure",le="0.001"} 0
route_queue_enqueue_latency_seconds_bucket{outcome="failure",le="0.0025"} 0
route_queue_enqueue_latency_seconds_bucket{outcome="failure",le="0.005"} 0
route_queue_enqueue_latency_seconds_bucket{outcome="failure",le="0.01"} 0
route_queue_enqueue_latency_seconds_bucket{outcome="failure",le="0.025"} 0
route_queue_enqueue_latency_seconds_bucket{outcome="failure",le="0.05"} 0
route_queue_enqueue_latency_seconds_bucket{outcome="failure",le="0.1"} 0
route_queue_enqueue_latency_seconds_bucket{outcome="failure",le="0.25"} 1
route_queue_enqueue_latency_seconds_bucket{outcome="failure",le="0.5"} 1
route_queue_enqueue_latency_seconds_bucket{outcome="failure",le="1"} 1
route_queue_enqueue_latency_seconds_bucket{outcome="failure",le="+Inf"} 1
route_queue_enqueue_latency_seconds_bucket{outcome="success",le="0.0005"} 0
route_queue_enqueue_latency_seconds_bucket{outcome="success",le="0.001"} 1
route_queue_enqueue_latency_seconds_bucket{outcome="success",le="0.0025"} 1
route_queue_enqueue_latency_seconds_bucket{outcome="success",le="0.005"} 1
route_queue_enqueue_latency_seconds_bucket{outcome="success",le="0.01"} 1
route_queue_enqueue_latency_seconds_bucket{outcome="success",le="0.025"} 1
route_queue_enqueue_latency_seconds_bucket{outcome="success",le="0.05"} 1
route_queue_enqueue_latency_seconds_bucket{outcome="success",le="0.1"} 1
route_queue_enqueue_latency_seconds_bucket{outcome="success",le="0.25"} 1
route_queue_enqueue_latency_seconds_bucket{outcome="success",le="0.5"} 1
route_queue_enqueue_latency_seconds_bucket{outcome="success",le="1"} 1
route_queue_enqueue_latency_seconds_bucket{outcome="success",le="+Inf"} 1
route_queue_enqueue_latency_seconds_count{outcome="failure"} 1
route_queue_enqueue_latency_seconds_count{outcome="success"} 1
route_queue_enqueue_latency_seconds_sum{outcome="failure"} 0.25
route_queue_enqueue_latency_seconds_sum{outcome="success"} 0.001
route_queue_enqueue_total{outcome="failure"} 1
route_queue_enqueue_total{outcome="success"} 1
"###
        );
    }

    proptest! {
        #[test]
        fn records_bounded_outcome_labels_and_latency(
            outcome in outcome_strategy(),
            latency_millis in 0_u64..=1_000,
        ) {
            let registry = Registry::new();
            let metrics = new_metrics(&registry).expect("metrics register");
            metrics.observe_enqueue(outcome, Duration::from_millis(latency_millis));

            let text = encode_route_queue_metrics(&registry);
            let label = outcome.as_label();
            let expected_total = format!("route_queue_enqueue_total{{outcome=\"{label}\"}} 1");
            let expected_count = format!(
                "route_queue_enqueue_latency_seconds_count{{outcome=\"{label}\"}} 1"
            );
            prop_assert!(text.contains(&expected_total));
            prop_assert!(text.contains(&expected_count));
            prop_assert!(!text.contains("outcome=\"unknown\""));
        }
    }

    fn spawn_metrics_registration(
        registry: Arc<Registry>,
    ) -> JoinHandle<Result<PrometheusRouteQueueMetrics, prometheus::Error>> {
        tokio::spawn(async move { new_metrics(&registry) })
    }

    fn new_metrics(registry: &Registry) -> Result<PrometheusRouteQueueMetrics, prometheus::Error> {
        PrometheusRouteQueueMetrics::new(registry)
    }

    fn outcome_strategy() -> impl Strategy<Value = RouteQueueOutcome> {
        prop_oneof![
            Just(RouteQueueOutcome::Success),
            Just(RouteQueueOutcome::Failure),
        ]
    }

    fn encode_route_queue_metrics(registry: &Registry) -> String {
        let mut buffer = Vec::new();
        if let Err(error) = prometheus::TextEncoder::new().encode(&registry.gather(), &mut buffer) {
            panic!("metrics should encode: {error}");
        }
        let text = match String::from_utf8(buffer) {
            Ok(text) => text,
            Err(error) => panic!("metrics text should be UTF-8: {error}"),
        };
        let mut lines = text
            .lines()
            .filter(|line| line.starts_with("route_queue_enqueue_"))
            .map(str::to_string)
            .collect::<Vec<_>>();
        lines.sort_by_key(|line| line.replace("le=\"+Inf\"", "le=\"z\""));
        lines.join("\n")
    }

    fn handle_registration_result(
        result: Result<PrometheusRouteQueueMetrics, prometheus::Error>,
    ) -> usize {
        match result {
            Ok(metrics) => {
                metrics.observe_enqueue(RouteQueueOutcome::Success, Duration::from_millis(1));
                1
            }
            Err(error) => {
                assert_duplicate_registration(error);
                0
            }
        }
    }

    fn assert_duplicate_registration(error: prometheus::Error) {
        let message = error.to_string();
        let normalized_message = message.to_lowercase();
        assert!(
            normalized_message.contains("already") || normalized_message.contains("duplicate"),
            "duplicate registration should be reported clearly: {message}"
        );
    }
}
