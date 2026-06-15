//! Versioned route-generation job payloads.

/// Versioned envelope for route-generation jobs.
#[derive(Clone, Debug, PartialEq)]
pub enum GenerateRouteJob {}

/// Version 1 payload for `GenerateRouteJob`.
#[derive(Clone, Debug, PartialEq)]
pub struct GenerateRouteJobV1 {}

/// Errors raised while building route-generation jobs from submissions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GenerateRouteJobBuildError {}

#[cfg(test)]
mod tests;
