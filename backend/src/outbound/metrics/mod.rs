//! Outbound adapters for metrics exporting.
//!
//! This module provides Prometheus-backed implementations of domain metrics
//! ports. All adapters here are feature-gated behind the `metrics` feature.

mod prometheus_idempotency;

pub use prometheus_idempotency::PrometheusIdempotencyMetrics;
