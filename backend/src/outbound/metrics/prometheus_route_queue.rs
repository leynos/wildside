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
