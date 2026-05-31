//! Domain port for observing runtime health.

use crate::domain::HealthObservation;

/// Observes process health without leaking adapter protocols into the domain.
pub trait HealthObserver {
    /// Report whether the process should be considered alive.
    fn observe_liveness(&self) -> HealthObservation;

    /// Report whether the process is ready to receive traffic.
    fn observe_readiness(&self) -> HealthObservation;
}
