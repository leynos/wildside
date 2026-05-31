//! Domain health observations for process liveness and readiness.
//!
//! The health model is intentionally small: it records whether the process
//! should be considered alive and whether it is ready to receive traffic. HTTP,
//! Kubernetes, Docker, and Helm adapters map these domain observations to their
//! own protocols.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::domain::ports::HealthObserver;

/// Health status reported by a domain health observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HealthStatus {
    /// The observed capability is available.
    Healthy,
    /// The observed capability is unavailable.
    Unhealthy,
}

impl HealthStatus {
    /// Return whether this status represents a healthy observation.
    ///
    /// # Examples
    ///
    /// ```
    /// use backend::domain::HealthStatus;
    ///
    /// assert!(HealthStatus::Healthy.is_healthy());
    /// assert!(!HealthStatus::Unhealthy.is_healthy());
    /// ```
    pub fn is_healthy(self) -> bool {
        matches!(self, Self::Healthy)
    }
}

/// A liveness or readiness observation owned by the domain layer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HealthObservation {
    status: HealthStatus,
}

impl HealthObservation {
    /// Build a healthy observation.
    ///
    /// # Examples
    ///
    /// ```
    /// use backend::domain::HealthObservation;
    ///
    /// assert!(HealthObservation::healthy().is_healthy());
    /// ```
    pub fn healthy() -> Self {
        Self {
            status: HealthStatus::Healthy,
        }
    }

    /// Build an unhealthy observation.
    ///
    /// # Examples
    ///
    /// ```
    /// use backend::domain::HealthObservation;
    ///
    /// assert!(!HealthObservation::unhealthy().is_healthy());
    /// ```
    pub fn unhealthy() -> Self {
        Self {
            status: HealthStatus::Unhealthy,
        }
    }

    /// Return this observation's status.
    pub fn status(self) -> HealthStatus {
        self.status
    }

    /// Return whether this observation is healthy.
    pub fn is_healthy(self) -> bool {
        self.status.is_healthy()
    }
}

/// Shared process health state used by runtime adapters.
///
/// New instances start live but not ready. The server composition root marks
/// readiness once the HTTP listener has been constructed.
pub struct ProcessHealth {
    ready: AtomicBool,
    live: AtomicBool,
}

impl Default for ProcessHealth {
    fn default() -> Self {
        Self {
            ready: AtomicBool::new(false),
            live: AtomicBool::new(true),
        }
    }
}

impl ProcessHealth {
    /// Create health state starting live but not ready.
    ///
    /// # Examples
    ///
    /// ```
    /// use backend::domain::ProcessHealth;
    /// use backend::domain::ports::HealthObserver;
    ///
    /// let health = ProcessHealth::new();
    /// assert!(health.observe_liveness().is_healthy());
    /// assert!(!health.observe_readiness().is_healthy());
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark the process as ready to serve traffic.
    pub fn mark_ready(&self) {
        self.ready.store(true, Ordering::Release);
    }

    /// Mark the process as not ready to serve traffic.
    pub fn mark_not_ready(&self) {
        self.ready.store(false, Ordering::Release);
    }

    /// Mark the process unhealthy so liveness checks fail.
    pub fn mark_unhealthy(&self) {
        self.live.store(false, Ordering::Release);
    }

    fn observation_from(is_healthy: bool) -> HealthObservation {
        if is_healthy {
            HealthObservation::healthy()
        } else {
            HealthObservation::unhealthy()
        }
    }
}

impl HealthObserver for ProcessHealth {
    fn observe_liveness(&self) -> HealthObservation {
        Self::observation_from(self.live.load(Ordering::Acquire))
    }

    fn observe_readiness(&self) -> HealthObservation {
        Self::observation_from(self.ready.load(Ordering::Acquire))
    }
}

#[cfg(test)]
mod tests {
    //! Tests for domain health observations and state transitions.

    use super::{HealthObservation, HealthStatus, ProcessHealth};
    use crate::domain::ports::HealthObserver;
    use rstest::{fixture, rstest};

    #[fixture]
    fn health() -> ProcessHealth {
        ProcessHealth::new()
    }

    #[rstest]
    fn default_health_starts_live_but_not_ready(health: ProcessHealth) {
        assert_eq!(
            health.observe_liveness().status(),
            HealthStatus::Healthy,
            "process should start live"
        );
        assert_eq!(
            health.observe_readiness().status(),
            HealthStatus::Unhealthy,
            "process should not start ready before runtime initialisation"
        );
    }

    #[rstest]
    fn marking_ready_makes_readiness_healthy(health: ProcessHealth) {
        health.mark_ready();

        assert!(health.observe_readiness().is_healthy());
    }

    #[rstest]
    fn marking_not_ready_makes_readiness_unhealthy(health: ProcessHealth) {
        health.mark_ready();
        health.mark_not_ready();

        assert!(!health.observe_readiness().is_healthy());
    }

    #[rstest]
    fn marking_unhealthy_makes_liveness_unhealthy(health: ProcessHealth) {
        health.mark_unhealthy();

        assert!(!health.observe_liveness().is_healthy());
    }

    #[rstest]
    #[case(HealthObservation::healthy(), HealthStatus::Healthy, true)]
    #[case(HealthObservation::unhealthy(), HealthStatus::Unhealthy, false)]
    fn observations_report_status_and_predicate(
        #[case] observation: HealthObservation,
        #[case] expected_status: HealthStatus,
        #[case] expected_healthy: bool,
    ) {
        assert_eq!(observation.status(), expected_status);
        assert_eq!(observation.is_healthy(), expected_healthy);
    }
}
