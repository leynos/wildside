//! Domain orchestration service for Overpass enrichment workers.
//!
//! The worker owns call admission (semaphore + quota + circuit breaker), retry
//! policy (jittered exponential backoff), and persistence through domain ports.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use mockable::Clock;
use tokio::sync::Semaphore;

use crate::domain::Error;
use crate::domain::ports::{
    EnrichmentJobFailure, EnrichmentJobFailureKind, EnrichmentJobMetrics, EnrichmentJobSuccess,
    EnrichmentProvenanceRecord, EnrichmentProvenanceRepository, OsmPoiRepository,
    OverpassEnrichmentRequest, OverpassEnrichmentResponse, OverpassEnrichmentSource,
};

mod attempt_error;
mod mapping;
mod policy;
mod runtime;

use attempt_error::AttemptError;
use policy::{AdmissionDecision, CircuitBreakerConfig, DailyQuota, WorkerPolicyState};
pub use runtime::{
    AttemptJitter, OverpassEnrichmentWorkerPorts, OverpassEnrichmentWorkerRuntime, TokioSleeper,
};

/// Worker configuration controlling quota, retries, and breaker behaviour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverpassEnrichmentWorkerConfig {
    /// Concurrent Overpass source call limit.
    pub max_concurrent_calls: usize,
    /// Daily request quota.
    pub max_daily_requests: u32,
    /// Daily transfer quota in bytes.
    pub max_daily_transfer_bytes: u64,
    /// Maximum source call attempts per job (including first call).
    pub max_attempts: u32,
    /// Initial retry backoff.
    pub initial_backoff: Duration,
    /// Maximum retry backoff cap.
    pub max_backoff: Duration,
    /// Consecutive failure threshold before opening the circuit.
    pub circuit_failure_threshold: u32,
    /// Open-state cooldown before allowing a half-open probe.
    pub circuit_open_cooldown: Duration,
}

impl Default for OverpassEnrichmentWorkerConfig {
    fn default() -> Self {
        Self {
            max_concurrent_calls: 2,
            max_daily_requests: 10_000,
            max_daily_transfer_bytes: 1_073_741_824,
            max_attempts: 3,
            initial_backoff: Duration::from_millis(200),
            max_backoff: Duration::from_secs(5),
            circuit_failure_threshold: 3,
            circuit_open_cooldown: Duration::from_secs(30),
        }
    }
}

/// Successful job execution summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverpassEnrichmentJobOutcome {
    /// Number of source attempts used for this job.
    pub attempts: u32,
    /// Number of persisted POIs.
    pub persisted_poi_count: usize,
    /// Transfer bytes consumed by the successful source call.
    pub transfer_bytes: u64,
}

/// Async clock-independent sleeping abstraction for retries.
#[async_trait]
pub trait EnrichmentSleeper: Send + Sync {
    /// Suspend execution for `duration`.
    ///
    /// ```rust,no_run
    /// use async_trait::async_trait;
    /// use backend::domain::EnrichmentSleeper;
    /// use std::sync::{Arc, Mutex};
    /// use std::time::Duration;
    /// #[derive(Default)]
    /// struct CountingSleeper {
    ///     calls: Arc<Mutex<u32>>,
    /// }
    /// #[async_trait]
    /// impl EnrichmentSleeper for CountingSleeper {
    ///     async fn sleep(&self, _duration: Duration) {
    ///         *self.calls.lock().expect("calls mutex") += 1;
    ///     }
    /// }
    /// # async fn demo() {
    /// let sleeper = CountingSleeper::default();
    /// sleeper.sleep(Duration::from_millis(25)).await;
    /// assert_eq!(*sleeper.calls.lock().expect("calls mutex"), 1);
    /// # }
    /// ```
    async fn sleep(&self, duration: Duration);
}

/// Retry backoff jitter abstraction.
pub trait BackoffJitter: Send + Sync {
    /// Return a jittered delay from the exponential base delay.
    ///
    /// ```rust
    /// use backend::domain::BackoffJitter;
    /// use chrono::{TimeZone, Utc};
    /// use std::time::Duration;
    /// struct DeterministicJitter;
    /// impl BackoffJitter for DeterministicJitter {
    ///     fn jittered_delay(&self, base: Duration, attempt: u32, _now: chrono::DateTime<chrono::Utc>) -> Duration {
    ///         base + Duration::from_millis(u64::from(attempt) * 5)
    ///     }
    /// }
    /// let jitter = DeterministicJitter;
    /// let now = Utc.with_ymd_and_hms(2026, 2, 26, 12, 0, 0).single().expect("valid time");
    /// let delay = jitter.jittered_delay(Duration::from_millis(100), 2, now);
    /// assert_eq!(delay, Duration::from_millis(110));
    /// ```
    fn jittered_delay(&self, base: Duration, attempt: u32, now: DateTime<Utc>) -> Duration;
}

/// Domain-owned Overpass enrichment worker.
pub struct OverpassEnrichmentWorker {
    source: Arc<dyn OverpassEnrichmentSource>,
    poi_repository: Arc<dyn OsmPoiRepository>,
    provenance_repository: Arc<dyn EnrichmentProvenanceRepository>,
    metrics: Arc<dyn EnrichmentJobMetrics>,
    clock: Arc<dyn Clock>,
    sleeper: Arc<dyn EnrichmentSleeper>,
    jitter: Arc<dyn BackoffJitter>,
    call_semaphore: Arc<Semaphore>,
    config: OverpassEnrichmentWorkerConfig,
    policy_state: Mutex<WorkerPolicyState>,
}

impl OverpassEnrichmentWorker {
    /// Build a worker using default runtime dependencies.
    /// ```rust,ignore
    /// let _worker = OverpassEnrichmentWorker::new(ports, clock, config);
    /// ```
    pub fn new(
        ports: OverpassEnrichmentWorkerPorts,
        clock: Arc<dyn Clock>,
        config: OverpassEnrichmentWorkerConfig,
    ) -> Self {
        Self::with_runtime(
            ports,
            clock,
            OverpassEnrichmentWorkerRuntime::default(),
            config,
        )
    }

    /// Build a worker with injected runtime abstractions.
    /// ```rust,ignore
    /// let _worker = OverpassEnrichmentWorker::with_runtime(ports, clock, runtime, config);
    /// ```
    pub fn with_runtime(
        ports: OverpassEnrichmentWorkerPorts,
        clock: Arc<dyn Clock>,
        runtime: OverpassEnrichmentWorkerRuntime,
        config: OverpassEnrichmentWorkerConfig,
    ) -> Self {
        let now = clock.utc();
        let policy_state = WorkerPolicyState::new(
            now,
            DailyQuota {
                max_requests_per_day: config.max_daily_requests,
                max_transfer_bytes_per_day: config.max_daily_transfer_bytes,
            },
            CircuitBreakerConfig {
                failure_threshold: config.circuit_failure_threshold,
                open_cooldown: config.circuit_open_cooldown,
            },
        );

        Self {
            source: ports.source,
            poi_repository: ports.poi_repository,
            provenance_repository: ports.provenance_repository,
            metrics: ports.metrics,
            clock,
            sleeper: runtime.sleeper,
            jitter: runtime.jitter,
            call_semaphore: Arc::new(Semaphore::new(config.max_concurrent_calls.max(1))),
            config,
            policy_state: Mutex::new(policy_state),
        }
    }

    /// Execute one enrichment job.
    /// ```rust,ignore
    /// let outcome = worker.process_job(request).await?;
    /// assert!(outcome.attempts >= 1);
    /// # Ok::<(), backend::domain::Error>(())
    /// ```
    pub async fn process_job(
        &self,
        request: OverpassEnrichmentRequest,
    ) -> Result<OverpassEnrichmentJobOutcome, Error> {
        let max_attempts = self.config.max_attempts.max(1);

        for attempt in 1..=max_attempts {
            match self.run_single_attempt(&request).await {
                Ok(report) => {
                    return self
                        .persist_and_record_success(&request, report, attempt)
                        .await;
                }
                Err(AttemptError::RetryableSource(_error)) if attempt < max_attempts => {
                    let base_delay = self.retry_base_delay(attempt);
                    let jittered =
                        self.jitter
                            .jittered_delay(base_delay, attempt, self.clock.utc());
                    self.sleeper.sleep(jittered).await;
                }
                Err(AttemptError::RetryableSource(error)) => {
                    self.record_failure_metric(EnrichmentJobFailureKind::RetryExhausted, attempt)
                        .await;
                    return Err(mapping::map_retry_exhausted_error(error));
                }
                Err(AttemptError::QuotaDenied(reason)) => {
                    let kind = mapping::map_quota_failure_kind(reason);
                    self.record_failure_metric(kind, attempt).await;
                    return Err(mapping::map_quota_error(reason));
                }
                Err(AttemptError::CircuitOpen) => {
                    self.record_failure_metric(EnrichmentJobFailureKind::CircuitOpen, attempt)
                        .await;
                    return Err(Error::service_unavailable(
                        "overpass enrichment circuit breaker is open",
                    ));
                }
                Err(AttemptError::SourceRejected(error)) => {
                    self.record_failure_metric(EnrichmentJobFailureKind::SourceRejected, attempt)
                        .await;
                    return Err(mapping::map_source_rejected_error(error));
                }
                Err(AttemptError::StateUnavailable(message)) => {
                    self.record_failure_metric(EnrichmentJobFailureKind::InternalError, attempt)
                        .await;
                    return Err(Error::internal(message));
                }
            }
        }

        Err(Error::internal(
            "unreachable enrichment control-flow state encountered",
        ))
    }

    async fn run_single_attempt(
        &self,
        request: &OverpassEnrichmentRequest,
    ) -> Result<OverpassEnrichmentResponse, AttemptError> {
        let _permit = self.call_semaphore.acquire().await.map_err(|_| {
            AttemptError::StateUnavailable("enrichment semaphore closed".to_owned())
        })?;

        let admission = {
            let mut state = self.policy_state.lock().map_err(|_| {
                AttemptError::StateUnavailable("worker policy state poisoned".to_owned())
            })?;
            state.admit_call(self.clock.utc())
        };

        match admission {
            AdmissionDecision::Allowed => {}
            AdmissionDecision::DeniedByQuota(reason) => {
                return Err(AttemptError::QuotaDenied(reason));
            }
            AdmissionDecision::DeniedByCircuit => {
                return Err(AttemptError::CircuitOpen);
            }
        }

        let source_result = self.source.fetch_pois(request).await;
        match source_result {
            Ok(report) => {
                let mut state = self.policy_state.lock().map_err(|_| {
                    AttemptError::StateUnavailable("worker policy state poisoned".to_owned())
                })?;
                state.record_success(self.clock.utc(), report.transfer_bytes);
                Ok(report)
            }
            Err(error) => {
                let mut state = self.policy_state.lock().map_err(|_| {
                    AttemptError::StateUnavailable("worker policy state poisoned".to_owned())
                })?;
                state.record_failure(self.clock.utc());

                if error.is_retryable() {
                    Err(AttemptError::RetryableSource(error))
                } else {
                    Err(AttemptError::SourceRejected(error))
                }
            }
        }
    }

    async fn persist_and_record_success(
        &self,
        request: &OverpassEnrichmentRequest,
        report: OverpassEnrichmentResponse,
        attempts: u32,
    ) -> Result<OverpassEnrichmentJobOutcome, Error> {
        let OverpassEnrichmentResponse {
            pois,
            transfer_bytes,
            source_url,
        } = report;
        let records = pois
            .into_iter()
            .map(mapping::map_overpass_poi)
            .collect::<Vec<_>>();

        if let Err(error) = self.poi_repository.upsert_pois(&records).await {
            self.record_failure_metric(EnrichmentJobFailureKind::PersistenceFailed, attempts)
                .await;
            return Err(mapping::map_persistence_error(error, attempts));
        }

        let provenance_record = EnrichmentProvenanceRecord {
            source_url,
            imported_at: self.clock.utc(),
            bounding_box: request.bounding_box,
        };
        if let Err(error) = self.provenance_repository.persist(&provenance_record).await {
            self.record_failure_metric(EnrichmentJobFailureKind::PersistenceFailed, attempts)
                .await;
            return Err(mapping::map_provenance_persistence_error(error, attempts));
        }

        self.record_success_metric(EnrichmentJobSuccess {
            attempt_count: attempts,
            persisted_poi_count: records.len(),
            transfer_bytes,
        })
        .await;

        Ok(OverpassEnrichmentJobOutcome {
            attempts,
            persisted_poi_count: records.len(),
            transfer_bytes,
        })
    }

    async fn record_success_metric(&self, payload: EnrichmentJobSuccess) {
        // Metrics exporter errors are deliberately non-fatal so a failed
        // counter write does not abort job processing throughput.
        let _ = self.metrics.record_success(&payload).await;
    }

    async fn record_failure_metric(&self, kind: EnrichmentJobFailureKind, attempts: u32) {
        let payload = EnrichmentJobFailure {
            attempt_count: attempts,
            kind,
        };
        // Metrics exporter errors are deliberately non-fatal so a failed
        // counter write does not abort job processing throughput.
        let _ = self.metrics.record_failure(&payload).await;
    }

    fn retry_base_delay(&self, attempt: u32) -> Duration {
        let exponent = 2_u32.saturating_pow(attempt.saturating_sub(1));
        let base_ms = u64::try_from(self.config.initial_backoff.as_millis()).unwrap_or(u64::MAX);
        let max_ms = u64::try_from(self.config.max_backoff.as_millis()).unwrap_or(u64::MAX);
        Duration::from_millis(base_ms.saturating_mul(u64::from(exponent)).min(max_ms))
    }
}

#[cfg(test)]
mod tests;
