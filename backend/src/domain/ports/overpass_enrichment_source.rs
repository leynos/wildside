//! Driven port for fetching POIs from the Overpass API.
//!
//! The domain owns the request shape and response contract so worker
//! orchestration can stay adapter-agnostic.

use std::collections::BTreeMap;

use async_trait::async_trait;
use uuid::Uuid;

use super::define_port_error;

/// Domain-owned enrichment request passed to the Overpass adapter.
#[derive(Debug, Clone, PartialEq)]
pub struct OverpassEnrichmentRequest {
    /// Stable job identifier for trace correlation.
    pub job_id: Uuid,
    /// Bounding box in `[min_lng, min_lat, max_lng, max_lat]` order.
    pub bounding_box: [f64; 4],
    /// Optional OSM tags used to scope the query.
    pub tags: Vec<String>,
}

/// One POI returned from Overpass.
#[derive(Debug, Clone, PartialEq)]
pub struct OverpassPoi {
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

/// Enrichment response payload produced by the Overpass adapter.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct OverpassEnrichmentResponse {
    /// POIs returned for the request.
    pub pois: Vec<OverpassPoi>,
    /// Estimated transfer size in bytes for quota accounting.
    pub transfer_bytes: u64,
}

define_port_error! {
    /// Errors surfaced while calling Overpass.
    pub enum OverpassEnrichmentSourceError {
        /// Network transport failed before receiving a response.
        Transport { message: String } =>
            "overpass transport failed: {message}",
        /// Overpass call exceeded timeout.
        Timeout { message: String } =>
            "overpass timeout: {message}",
        /// Overpass rate-limited the request.
        RateLimited { message: String } =>
            "overpass rate limited request: {message}",
        /// Overpass response could not be decoded.
        Decode { message: String } =>
            "overpass response decode failed: {message}",
        /// Adapter rejected request before execution.
        InvalidRequest { message: String } =>
            "overpass request invalid: {message}",
    }
}

impl OverpassEnrichmentSourceError {
    /// Return whether retrying this error is expected to help.
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
pub trait OverpassEnrichmentSource: Send + Sync {
    /// Fetch POIs for one enrichment request.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use uuid::Uuid;
    ///
    /// use backend::domain::ports::{
    ///     FixtureOverpassEnrichmentSource, OverpassEnrichmentRequest,
    ///     OverpassEnrichmentSource,
    /// };
    ///
    /// let source = FixtureOverpassEnrichmentSource;
    /// let response = source
    ///     .fetch_pois(&OverpassEnrichmentRequest {
    ///         job_id: Uuid::new_v4(),
    ///         bounding_box: [-3.30, 55.90, -3.10, 56.00],
    ///         tags: vec!["amenity".to_owned()],
    ///     })
    ///     .await?;
    /// assert!(response.pois.is_empty());
    /// # Ok::<(), backend::domain::ports::OverpassEnrichmentSourceError>(())
    /// ```
    async fn fetch_pois(
        &self,
        request: &OverpassEnrichmentRequest,
    ) -> Result<OverpassEnrichmentResponse, OverpassEnrichmentSourceError>;
}

/// Fixture implementation returning an empty response.
#[derive(Debug, Clone, Copy, Default)]
pub struct FixtureOverpassEnrichmentSource;

#[async_trait]
impl OverpassEnrichmentSource for FixtureOverpassEnrichmentSource {
    async fn fetch_pois(
        &self,
        _request: &OverpassEnrichmentRequest,
    ) -> Result<OverpassEnrichmentResponse, OverpassEnrichmentSourceError> {
        Ok(OverpassEnrichmentResponse::default())
    }
}
