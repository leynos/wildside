//! Attempt-local error outcomes for one Overpass source call.
//!
//! These states let retry, quota, and circuit decisions remain explicit inside
//! the worker loop without leaking attempt-control details into the public API.

use crate::domain::ports::EnrichmentSourceError;

use super::policy::QuotaDenyReason;

pub(super) enum AttemptError {
    RetryableSource(EnrichmentSourceError),
    SourceRejected(EnrichmentSourceError),
    QuotaDenied(QuotaDenyReason),
    CircuitOpen,
    StateUnavailable(String),
}
