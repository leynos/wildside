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
    /// Worker runtime state was unavailable (for example poisoned mutexes).
    InternalError,
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
    ///
    /// ```rust,ignore
    /// use backend::domain::ports::{
    ///     EnrichmentJobMetrics, EnrichmentJobMetricsError, EnrichmentJobSuccess,
    ///     NoOpEnrichmentJobMetrics,
    /// };
    ///
    /// # async fn demo() {
    /// let metrics = NoOpEnrichmentJobMetrics;
    /// let payload = EnrichmentJobSuccess {
    ///     attempt_count: 1,
    ///     persisted_poi_count: 3,
    ///     transfer_bytes: 512,
    /// };
    ///
    /// let result = metrics.record_success(&payload).await;
    /// assert!(result.is_ok());
    /// let _ = Ok::<(), EnrichmentJobMetricsError>(());
    /// # }
    /// ```
    async fn record_success(
        &self,
        payload: &EnrichmentJobSuccess,
    ) -> Result<(), EnrichmentJobMetricsError>;

    /// Record a failed enrichment job run.
    ///
    /// ```rust,ignore
    /// use backend::domain::ports::{
    ///     EnrichmentJobFailure, EnrichmentJobFailureKind, EnrichmentJobMetrics,
    ///     EnrichmentJobMetricsError, NoOpEnrichmentJobMetrics,
    /// };
    ///
    /// # async fn demo() {
    /// let metrics = NoOpEnrichmentJobMetrics;
    /// let payload = EnrichmentJobFailure {
    ///     attempt_count: 2,
    ///     kind: EnrichmentJobFailureKind::RetryExhausted,
    /// };
    ///
    /// let result = metrics.record_failure(&payload).await;
    /// assert!(result.is_ok());
    /// let _ = Ok::<(), EnrichmentJobMetricsError>(());
    /// # }
    /// ```
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
