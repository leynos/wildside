//! Versioned enrichment job payloads.

/// Versioned envelope for enrichment jobs.
#[derive(Clone, Debug, PartialEq)]
pub enum EnrichmentJob {}

/// Version 1 payload for `EnrichmentJob`.
#[derive(Clone, Debug, PartialEq)]
pub struct EnrichmentJobV1 {}

/// Errors raised while building enrichment jobs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnrichmentJobBuildError {}

#[cfg(test)]
mod tests;
