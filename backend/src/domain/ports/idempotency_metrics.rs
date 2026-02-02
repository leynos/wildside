//! Domain port surface for recording idempotency request audit metrics.
//!
//! This port enables observability of idempotency request outcomes without
//! coupling domain logic to a specific metrics backend. Implementations may
//! export to Prometheus, log to structured output, or simply discard metrics
//! in tests.

use async_trait::async_trait;

use super::define_port_error;

define_port_error! {
    /// Errors exposed when recording idempotency metrics.
    pub enum IdempotencyMetricsError {
        /// Metric exporter rejected the write.
        Export { message: String } => "idempotency metrics exporter failed: {message}",
    }
}

/// Labels for idempotency metric recording.
///
/// These labels are attached to every metrics write to enable aggregation
/// and filtering in the metrics backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdempotencyMetricLabels {
    /// Anonymized user scope (first 8 hex chars of SHA-256 hash of user ID).
    pub user_scope: String,
    /// Age bucket of the idempotency key (e.g., "0-1m", "1-5m").
    /// `None` for misses (no prior key exists).
    pub age_bucket: Option<String>,
}

/// Metrics recording port for idempotency request outcomes.
///
/// Implementors record hits (replayed responses), misses (new requests),
/// and conflicts (same key, different payload) with associated labels.
#[async_trait]
pub trait IdempotencyMetrics: Send + Sync {
    /// Record an idempotency miss (new request, no existing key).
    async fn record_miss(
        &self,
        labels: &IdempotencyMetricLabels,
    ) -> Result<(), IdempotencyMetricsError>;

    /// Record an idempotency hit (replay of existing matching request).
    async fn record_hit(
        &self,
        labels: &IdempotencyMetricLabels,
    ) -> Result<(), IdempotencyMetricsError>;

    /// Record an idempotency conflict (same key, different payload).
    async fn record_conflict(
        &self,
        labels: &IdempotencyMetricLabels,
    ) -> Result<(), IdempotencyMetricsError>;
}

/// No-op implementation for when metrics are disabled or in tests.
///
/// All methods immediately return `Ok(())` without side effects.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoOpIdempotencyMetrics;

#[async_trait]
impl IdempotencyMetrics for NoOpIdempotencyMetrics {
    async fn record_miss(
        &self,
        _labels: &IdempotencyMetricLabels,
    ) -> Result<(), IdempotencyMetricsError> {
        Ok(())
    }

    async fn record_hit(
        &self,
        _labels: &IdempotencyMetricLabels,
    ) -> Result<(), IdempotencyMetricsError> {
        Ok(())
    }

    async fn record_conflict(
        &self,
        _labels: &IdempotencyMetricLabels,
    ) -> Result<(), IdempotencyMetricsError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    //! Ensures NoOpIdempotencyMetrics accepts miss, hit, and conflict events.
    use super::*;

    #[tokio::test]
    async fn noop_record_miss_returns_ok() {
        let metrics = NoOpIdempotencyMetrics;
        let labels = IdempotencyMetricLabels {
            user_scope: "a1b2c3d4".to_string(),
            age_bucket: None,
        };
        assert!(metrics.record_miss(&labels).await.is_ok());
    }

    #[tokio::test]
    async fn noop_record_hit_returns_ok() {
        let metrics = NoOpIdempotencyMetrics;
        let labels = IdempotencyMetricLabels {
            user_scope: "a1b2c3d4".to_string(),
            age_bucket: Some("1-5m".to_string()),
        };
        assert!(metrics.record_hit(&labels).await.is_ok());
    }

    #[tokio::test]
    async fn noop_record_conflict_returns_ok() {
        let metrics = NoOpIdempotencyMetrics;
        let labels = IdempotencyMetricLabels {
            user_scope: "a1b2c3d4".to_string(),
            age_bucket: Some("5-30m".to_string()),
        };
        assert!(metrics.record_conflict(&labels).await.is_ok());
    }

    #[test]
    fn error_constructor_accepts_str() {
        let err = IdempotencyMetricsError::export("test error");
        assert_eq!(
            err.to_string(),
            "idempotency metrics exporter failed: test error"
        );
    }
}
