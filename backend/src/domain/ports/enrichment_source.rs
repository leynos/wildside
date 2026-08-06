//! Driven port for fetching POIs from an enrichment source.
//!
//! The domain owns the request shape and response contract so worker
//! orchestration can stay adapter-agnostic. Nothing here names a specific
//! provider: the Overpass adapter in `crate::outbound::overpass` translates
//! [`EnrichmentRequest`] into a provider query and maps provider failures onto
//! [`EnrichmentSourceError`].

use std::collections::BTreeMap;

use async_trait::async_trait;
use uuid::Uuid;

use super::define_port_error;
use crate::domain::BoundingBox;

/// Domain-owned enrichment request passed to an enrichment adapter.
#[derive(Debug, Clone, PartialEq)]
pub struct EnrichmentRequest {
    /// Stable job identifier for trace correlation.
    pub job_id: Uuid,
    /// Validated WGS84 bounding box to enrich.
    pub bounding_box: BoundingBox,
    /// Optional OSM tags used to scope the query.
    pub tags: Vec<String>,
}

/// One POI returned from an enrichment source.
#[derive(Debug, Clone, PartialEq)]
pub struct EnrichmentPoi {
    /// OSM element type (`node`, `way`, or `relation`).
    pub element_type: String,
    /// Raw OSM element identifier.
    pub element_id: i64,
    /// Longitude in WGS84.
    pub longitude: f64,
    /// Latitude in WGS84.
    pub latitude: f64,
    /// Raw OSM tags.
    pub tags: BTreeMap<String, String>,
}

/// Enrichment response payload produced by an enrichment adapter.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EnrichmentResponse {
    /// POIs returned for the request.
    pub pois: Vec<EnrichmentPoi>,
    /// Estimated transfer size in bytes for quota accounting.
    pub transfer_bytes: u64,
    /// Source URL used by the adapter for this response.
    pub source_url: String,
}

define_port_error! {
    /// Errors surfaced while calling an enrichment source.
    pub enum EnrichmentSourceError {
        /// Network transport failed before receiving a response.
        Transport { message: String } =>
            "enrichment transport failed: {message}",
        /// The enrichment call exceeded its timeout.
        Timeout { message: String } =>
            "enrichment timeout: {message}",
        /// The enrichment source rate-limited the request.
        RateLimited { message: String } =>
            "enrichment source rate limited request: {message}",
        /// The enrichment response could not be decoded.
        Decode { message: String } =>
            "enrichment response decode failed: {message}",
        /// Adapter rejected request before execution.
        InvalidRequest { message: String } =>
            "enrichment request invalid: {message}",
    }
}

impl EnrichmentSourceError {
    /// Return whether retrying this error is expected to help.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use backend::domain::ports::EnrichmentSourceError;
    ///
    /// assert!(EnrichmentSourceError::transport("temporary").is_retryable());
    /// assert!(!EnrichmentSourceError::invalid_request("bad bbox").is_retryable());
    /// ```
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Transport { .. } | Self::Timeout { .. } | Self::RateLimited { .. }
        )
    }
}

/// Port for querying Overpass for enrichment POIs.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait EnrichmentSource: Send + Sync {
    /// Fetch POIs for one enrichment request.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use uuid::Uuid;
    ///
    /// use backend::domain::ports::{
    ///     FixtureEnrichmentSource, EnrichmentRequest,
    ///     EnrichmentSource,
    /// };
    ///
    /// let source = FixtureEnrichmentSource;
    /// let response = source
    ///     .fetch_pois(&EnrichmentRequest {
    ///         job_id: Uuid::new_v4(),
    ///         bounding_box: [-3.30, 55.90, -3.10, 56.00],
    ///         tags: vec!["amenity".to_owned()],
    ///     })
    ///     .await?;
    /// assert!(response.pois.is_empty());
    /// # Ok::<(), backend::domain::ports::EnrichmentSourceError>(())
    /// ```
    async fn fetch_pois(
        &self,
        request: &EnrichmentRequest,
    ) -> Result<EnrichmentResponse, EnrichmentSourceError>;
}

/// Fixture implementation returning an empty response.
#[derive(Debug, Clone, Copy, Default)]
pub struct FixtureEnrichmentSource;

#[async_trait]
impl EnrichmentSource for FixtureEnrichmentSource {
    async fn fetch_pois(
        &self,
        _request: &EnrichmentRequest,
    ) -> Result<EnrichmentResponse, EnrichmentSourceError> {
        Ok(EnrichmentResponse::default())
    }
}
