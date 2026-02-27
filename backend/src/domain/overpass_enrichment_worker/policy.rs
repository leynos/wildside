//! Policy state machine for Overpass enrichment worker admission.
//!
//! This module contains adapter-agnostic policy logic for:
//! - daily quota enforcement (requests and transfer bytes);
//! - circuit breaker transitions (closed/open/half-open).

use std::time::Duration;

use chrono::{DateTime, NaiveDate, Utc};

/// Daily call quota limits for Overpass enrichment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DailyQuota {
    /// Maximum allowed source call attempts per UTC day.
    pub max_requests_per_day: u32,
    /// Maximum allowed transfer bytes per UTC day.
    pub max_transfer_bytes_per_day: u64,
}

/// Circuit breaker configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CircuitBreakerConfig {
    /// Consecutive failures required to open the breaker.
    pub failure_threshold: u32,
    /// Cooldown period while the breaker remains open.
    pub open_cooldown: Duration,
}

/// Quota denial reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaDenyReason {
    /// Daily request limit has been reached.
    RequestLimit,
    /// Daily transfer limit has been reached.
    TransferLimit,
}

/// Circuit breaker state.
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitBreakerState {
    /// Normal operation.
    Closed,
    /// Calls are blocked until cooldown elapses.
    Open,
    /// One probe call is allowed.
    HalfOpen,
}

/// Admission decision for one outbound source call attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdmissionDecision {
    /// Call is admitted and quota request count was reserved.
    Allowed,
    /// Daily quota denied the call.
    DeniedByQuota(QuotaDenyReason),
    /// Circuit breaker denied the call.
    DeniedByCircuit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CircuitInternalState {
    Closed { consecutive_failures: u32 },
    Open { opened_at: DateTime<Utc> },
    HalfOpen { probe_in_flight: bool },
}

/// Mutable policy state shared across worker calls.
#[derive(Debug, Clone)]
pub struct WorkerPolicyState {
    quota: DailyQuota,
    quota_day: NaiveDate,
    requests_used: u32,
    transfer_bytes_used: u64,
    circuit_config: CircuitBreakerConfig,
    circuit_state: CircuitInternalState,
}

impl WorkerPolicyState {
    /// Build policy state rooted at the provided clock instant.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use std::time::Duration;
    /// use chrono::{TimeZone, Utc};
    /// use backend::domain::overpass_enrichment_worker::policy::{
    ///     CircuitBreakerConfig, DailyQuota, WorkerPolicyState,
    /// };
    ///
    /// let now = Utc.with_ymd_and_hms(2026, 2, 26, 12, 0, 0).single().expect("valid");
    /// let _state = WorkerPolicyState::new(
    ///     now,
    ///     DailyQuota {
    ///         max_requests_per_day: 100,
    ///         max_transfer_bytes_per_day: 1_024,
    ///     },
    ///     CircuitBreakerConfig {
    ///         failure_threshold: 3,
    ///         open_cooldown: Duration::from_secs(30),
    ///     },
    /// );
    /// ```
    pub fn new(now: DateTime<Utc>, quota: DailyQuota, circuit: CircuitBreakerConfig) -> Self {
        Self {
            quota,
            quota_day: now.date_naive(),
            requests_used: 0,
            transfer_bytes_used: 0,
            circuit_config: CircuitBreakerConfig {
                failure_threshold: circuit.failure_threshold.max(1),
                open_cooldown: circuit.open_cooldown,
            },
            circuit_state: CircuitInternalState::Closed {
                consecutive_failures: 0,
            },
        }
    }

    /// Attempt to admit one source call.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use std::time::Duration;
    /// use chrono::{TimeZone, Utc};
    /// use backend::domain::overpass_enrichment_worker::policy::{
    ///     AdmissionDecision, CircuitBreakerConfig, DailyQuota, WorkerPolicyState,
    /// };
    ///
    /// let now = Utc.with_ymd_and_hms(2026, 2, 26, 12, 0, 0).single().expect("valid");
    /// let mut state = WorkerPolicyState::new(
    ///     now,
    ///     DailyQuota {
    ///         max_requests_per_day: 1,
    ///         max_transfer_bytes_per_day: 1_024,
    ///     },
    ///     CircuitBreakerConfig {
    ///         failure_threshold: 2,
    ///         open_cooldown: Duration::from_secs(30),
    ///     },
    /// );
    ///
    /// assert_eq!(state.admit_call(now), AdmissionDecision::Allowed);
    /// assert!(matches!(
    ///     state.admit_call(now),
    ///     AdmissionDecision::DeniedByQuota(_)
    /// ));
    /// ```
    pub fn admit_call(&mut self, now: DateTime<Utc>) -> AdmissionDecision {
        self.reset_day_if_needed(now);

        if self.requests_used >= self.quota.max_requests_per_day {
            return AdmissionDecision::DeniedByQuota(QuotaDenyReason::RequestLimit);
        }
        if self.transfer_bytes_used >= self.quota.max_transfer_bytes_per_day {
            return AdmissionDecision::DeniedByQuota(QuotaDenyReason::TransferLimit);
        }

        match self.circuit_state {
            CircuitInternalState::Closed { .. } => {
                self.requests_used = self.requests_used.saturating_add(1);
                AdmissionDecision::Allowed
            }
            CircuitInternalState::Open { opened_at }
                if is_cooldown_elapsed(opened_at, now, self.circuit_config.open_cooldown) =>
            {
                self.circuit_state = CircuitInternalState::HalfOpen {
                    probe_in_flight: true,
                };
                self.requests_used = self.requests_used.saturating_add(1);
                AdmissionDecision::Allowed
            }
            CircuitInternalState::Open { .. } => AdmissionDecision::DeniedByCircuit,
            CircuitInternalState::HalfOpen { probe_in_flight } => {
                if probe_in_flight {
                    AdmissionDecision::DeniedByCircuit
                } else {
                    self.circuit_state = CircuitInternalState::HalfOpen {
                        probe_in_flight: true,
                    };
                    self.requests_used = self.requests_used.saturating_add(1);
                    AdmissionDecision::Allowed
                }
            }
        }
    }

    /// Record a successful source call.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use std::time::Duration;
    /// use chrono::{TimeZone, Utc};
    /// use backend::domain::overpass_enrichment_worker::policy::{
    ///     CircuitBreakerConfig, DailyQuota, WorkerPolicyState,
    /// };
    ///
    /// let now = Utc.with_ymd_and_hms(2026, 2, 26, 12, 0, 0).single().expect("valid");
    /// let mut state = WorkerPolicyState::new(
    ///     now,
    ///     DailyQuota {
    ///         max_requests_per_day: 10,
    ///         max_transfer_bytes_per_day: 1_024,
    ///     },
    ///     CircuitBreakerConfig {
    ///         failure_threshold: 1,
    ///         open_cooldown: Duration::from_secs(30),
    ///     },
    /// );
    /// let _ = state.admit_call(now);
    /// state.record_success(now, 512);
    /// ```
    pub fn record_success(&mut self, now: DateTime<Utc>, transfer_bytes: u64) {
        self.reset_day_if_needed(now);
        self.transfer_bytes_used = self.transfer_bytes_used.saturating_add(transfer_bytes);

        self.circuit_state = CircuitInternalState::Closed {
            consecutive_failures: 0,
        };
    }

    /// Record a failed source call.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use std::time::Duration;
    /// use chrono::{TimeZone, Utc};
    /// use backend::domain::overpass_enrichment_worker::policy::{
    ///     AdmissionDecision, CircuitBreakerConfig, DailyQuota, WorkerPolicyState,
    /// };
    ///
    /// let now = Utc.with_ymd_and_hms(2026, 2, 26, 12, 0, 0).single().expect("valid");
    /// let mut state = WorkerPolicyState::new(
    ///     now,
    ///     DailyQuota {
    ///         max_requests_per_day: 10,
    ///         max_transfer_bytes_per_day: 1_024,
    ///     },
    ///     CircuitBreakerConfig {
    ///         failure_threshold: 1,
    ///         open_cooldown: Duration::from_secs(30),
    ///     },
    /// );
    /// assert_eq!(state.admit_call(now), AdmissionDecision::Allowed);
    /// state.record_failure(now);
    /// assert!(matches!(state.admit_call(now), AdmissionDecision::DeniedByCircuit));
    /// ```
    pub fn record_failure(&mut self, now: DateTime<Utc>) {
        self.reset_day_if_needed(now);

        self.circuit_state = match self.circuit_state {
            CircuitInternalState::Closed {
                consecutive_failures,
            } => {
                let next_failures = consecutive_failures.saturating_add(1);
                if next_failures >= self.circuit_config.failure_threshold {
                    CircuitInternalState::Open { opened_at: now }
                } else {
                    CircuitInternalState::Closed {
                        consecutive_failures: next_failures,
                    }
                }
            }
            CircuitInternalState::HalfOpen { .. } => CircuitInternalState::Open { opened_at: now },
            CircuitInternalState::Open { opened_at } => CircuitInternalState::Open { opened_at },
        };
    }

    /// Snapshot current circuit breaker state.
    #[cfg(test)]
    pub fn circuit_state(&self) -> CircuitBreakerState {
        match self.circuit_state {
            CircuitInternalState::Closed { .. } => CircuitBreakerState::Closed,
            CircuitInternalState::Open { .. } => CircuitBreakerState::Open,
            CircuitInternalState::HalfOpen { .. } => CircuitBreakerState::HalfOpen,
        }
    }

    fn reset_day_if_needed(&mut self, now: DateTime<Utc>) {
        let now_day = now.date_naive();
        if now_day > self.quota_day {
            self.quota_day = now_day;
            self.requests_used = 0;
            self.transfer_bytes_used = 0;
        }
    }
}

fn is_cooldown_elapsed(opened_at: DateTime<Utc>, now: DateTime<Utc>, cooldown: Duration) -> bool {
    // Fail open when std->chrono conversion fails: this path is unlikely, and
    // returning true avoids accidentally holding the circuit open forever.
    let Ok(cooldown) = chrono::Duration::from_std(cooldown) else {
        return true;
    };

    now >= opened_at + cooldown
}
