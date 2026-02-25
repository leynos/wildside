//! Driven port for ingestion provenance and deterministic rerun keys.

use chrono::{DateTime, Utc};

use async_trait::async_trait;

use super::{OsmPoiIngestionRecord, define_port_error};

/// Persisted provenance record for one geofenced OSM ingestion run.
#[derive(Debug, Clone, PartialEq)]
pub struct OsmIngestionProvenanceRecord {
    /// Logical geofence identifier.
    pub geofence_id: String,
    /// Source URL used to obtain the input.
    pub source_url: String,
    /// Stable digest of the input payload.
    pub input_digest: String,
    /// Timestamp for when this ingest was materialized.
    pub imported_at: DateTime<Utc>,
    /// Geofence bounds `[min_lng, min_lat, max_lng, max_lat]`.
    pub geofence_bounds: [f64; 4],
    /// Number of POIs reported by the upstream source.
    pub raw_poi_count: u64,
    /// Number of POIs persisted after geofence filtering.
    pub filtered_poi_count: u64,
}

define_port_error! {
    /// Errors raised while reading/writing provenance rows.
    pub enum OsmIngestionProvenanceRepositoryError {
        /// Repository connection could not be established.
        Connection { message: String } =>
            "osm ingestion provenance connection failed: {message}",
        /// Query or mutation failed during execution.
        Query { message: String } =>
            "osm ingestion provenance query failed: {message}",
        /// Unique rerun key conflict occurred during insert.
        Conflict { message: String } =>
            "osm ingestion provenance conflict: {message}",
    }
}

/// Port for deterministic rerun key lookup and provenance persistence.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait OsmIngestionProvenanceRepository: Send + Sync {
    /// Lookup a persisted run by `(geofence_id, input_digest)`.
    async fn find_by_rerun_key(
        &self,
        geofence_id: &str,
        input_digest: &str,
    ) -> Result<Option<OsmIngestionProvenanceRecord>, OsmIngestionProvenanceRepositoryError>;

    /// Persist one ingestion batch atomically.
    ///
    /// Implementations must write the provenance row and filtered POIs within
    /// one transaction boundary so rerun-key conflicts and write failures do
    /// not leave partial ingest state behind.
    async fn persist_ingestion(
        &self,
        provenance: &OsmIngestionProvenanceRecord,
        poi_records: &[OsmPoiIngestionRecord],
    ) -> Result<(), OsmIngestionProvenanceRepositoryError>;
}

/// Fixture repository implementation for tests without persistence coupling.
#[derive(Debug, Clone, Copy, Default)]
pub struct FixtureOsmIngestionProvenanceRepository;

#[async_trait]
impl OsmIngestionProvenanceRepository for FixtureOsmIngestionProvenanceRepository {
    async fn find_by_rerun_key(
        &self,
        _geofence_id: &str,
        _input_digest: &str,
    ) -> Result<Option<OsmIngestionProvenanceRecord>, OsmIngestionProvenanceRepositoryError> {
        Ok(None)
    }

    async fn persist_ingestion(
        &self,
        _provenance: &OsmIngestionProvenanceRecord,
        _poi_records: &[OsmPoiIngestionRecord],
    ) -> Result<(), OsmIngestionProvenanceRepositoryError> {
        Ok(())
    }
}
