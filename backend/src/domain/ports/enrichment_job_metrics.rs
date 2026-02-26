//! Domain port surface for enrichment job success/failure counters.
//!
//! This keeps enrichment observability at the domain boundary so adapters can
//! emit Prometheus counters without leaking implementation details into worker
//! orchestration.

use async_trait::async_trait;

use super::define_port_error;

define_port_error! {
    /// Errors exposed when recording enrichment metrics.
    pub enum EnrichmentJobMetricsError {
        /// Metric exporter rejected the write.
        Export { message: String } =>
            "enrichment metrics exporter failed: {message}",
    }
}

/// Failure reason labels for enrichment jobs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum EnrichmentJobFailureKind {
    /// Request quota (`requests/day`) prevented the Overpass call.
    QuotaRequestLimit,
    /// Transfer quota (`bytes/day`) prevented the Overpass call.
    QuotaTransferLimit,
    /// Circuit breaker was open and short-circuited the call.
    CircuitOpen,
    /// Retry budget was exhausted for retryable source errors.
    RetryExhausted,
    /// Source returned a non-retryable failure.
    SourceRejected,
    /// Persistence adapter could not upsert fetched POIs.
    PersistenceFailed,
}

/// Success metric payload for one job execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnrichmentJobSuccess {
    /// Number of source call attempts used by this job.
    pub attempt_count: u32,
    /// Number of POIs persisted through `OsmPoiRepository`.
    pub persisted_poi_count: usize,
    /// Bytes counted against transfer quota.
    pub transfer_bytes: u64,
}

/// Failure metric payload for one job execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnrichmentJobFailure {
    /// Number of source call attempts used by this job.
    pub attempt_count: u32,
    /// Domain-level failure reason label.
    pub kind: EnrichmentJobFailureKind,
}

/// Metrics recording port for enrichment job counters.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait EnrichmentJobMetrics: Send + Sync {
    /// Record a successful enrichment job run.
    async fn record_success(
        &self,
        payload: &EnrichmentJobSuccess,
    ) -> Result<(), EnrichmentJobMetricsError>;

    /// Record a failed enrichment job run.
    async fn record_failure(
        &self,
        payload: &EnrichmentJobFailure,
    ) -> Result<(), EnrichmentJobMetricsError>;
}

/// No-op implementation used when metrics are disabled or in tests.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoOpEnrichmentJobMetrics;

#[async_trait]
impl EnrichmentJobMetrics for NoOpEnrichmentJobMetrics {
    async fn record_success(
        &self,
        _payload: &EnrichmentJobSuccess,
    ) -> Result<(), EnrichmentJobMetricsError> {
        Ok(())
    }

    async fn record_failure(
        &self,
        _payload: &EnrichmentJobFailure,
    ) -> Result<(), EnrichmentJobMetricsError> {
        Ok(())
    }
}
