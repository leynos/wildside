//! Cross-layer observability helpers shared by inbound and outbound adapters.
//!
//! Modules here own logging and Prometheus side-effects so the domain layer
//! can stay free of `tracing`, `prometheus`, and process-global metric
//! collectors. Both inbound HTTP handlers and outbound persistence error
//! mappers record pagination cursor failures through this module.

pub(crate) mod pagination_errors;
