//! Driven port for persisting geofence-filtered OSM POIs.

use std::collections::BTreeMap;

use async_trait::async_trait;

use super::define_port_error;

/// Domain-owned POI payload persisted by ingestion adapters.
#[derive(Debug, Clone, PartialEq)]
pub struct OsmPoiIngestionRecord {
    /// OSM element type (`node`, `way`, or `relation`).
    pub element_type: String,
    /// Raw OSM element identifier.
    pub element_id: i64,
    /// Longitude in WGS84.
    pub longitude: f64,
    /// Latitude in WGS84.
    pub latitude: f64,
    /// Raw OSM tags for the element.
    pub tags: BTreeMap<String, String>,
}

define_port_error! {
    /// Errors raised while persisting OSM POIs.
    pub enum OsmPoiRepositoryError {
        /// Repository connection could not be established.
        Connection { message: String } =>
            "osm poi persistence connection failed: {message}",
        /// Query or mutation failed during execution.
        Query { message: String } =>
            "osm poi persistence query failed: {message}",
    }
}

/// Port for writing OSM POIs to the backend store.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait OsmPoiRepository: Send + Sync {
    /// Upsert POIs keyed by `(element_type, element_id)`.
    async fn upsert_pois(
        &self,
        records: &[OsmPoiIngestionRecord],
    ) -> Result<(), OsmPoiRepositoryError>;
}

/// Fixture implementation for tests that do not exercise persistence.
#[derive(Debug, Clone, Copy, Default)]
pub struct FixtureOsmPoiRepository;

#[async_trait]
impl OsmPoiRepository for FixtureOsmPoiRepository {
    async fn upsert_pois(
        &self,
        _records: &[OsmPoiIngestionRecord],
    ) -> Result<(), OsmPoiRepositoryError> {
        Ok(())
    }
}
