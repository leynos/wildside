//! Outbound adapters for metrics exporting.
//!
//! This module provides Prometheus-backed implementations of domain metrics
//! ports. All adapters here are feature-gated behind the `metrics` feature.

mod prometheus_enrichment_jobs;
mod prometheus_idempotency;
mod prometheus_route_queue;

pub use prometheus_enrichment_jobs::PrometheusEnrichmentJobMetrics;
pub use prometheus_idempotency::PrometheusIdempotencyMetrics;
pub use prometheus_route_queue::PrometheusRouteQueueMetrics;
