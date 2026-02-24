//! Driving port for backend-owned OSM ingestion orchestration.

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use async_trait::async_trait;

use crate::domain::Error;

/// Command request for one OSM ingestion run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OsmIngestionRequest {
    /// Path to the `.osm.pbf` input file.
    pub osm_pbf_path: PathBuf,
    /// Logical source URL used for provenance tracking.
    pub source_url: String,
    /// Geofence identifier for rerun keying.
    pub geofence_id: String,
    /// Geofence bounds `[min_lng, min_lat, max_lng, max_lat]`.
    pub geofence_bounds: [f64; 4],
    /// Stable digest of the input payload.
    pub input_digest: String,
}

/// Execution outcome status for an ingestion command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OsmIngestionStatus {
    /// Data was ingested and persisted in this invocation.
    Executed,
    /// Existing provenance matched and the run was treated as deterministic replay.
    Replayed,
}

/// Command response for one OSM ingestion run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OsmIngestionOutcome {
    /// Command execution status.
    pub status: OsmIngestionStatus,
    /// Source URL captured in provenance.
    pub source_url: String,
    /// Geofence identifier used for the run.
    pub geofence_id: String,
    /// Stable digest of the input payload.
    pub input_digest: String,
    /// Timestamp persisted with provenance.
    pub imported_at: DateTime<Utc>,
    /// Geofence bounds `[min_lng, min_lat, max_lng, max_lat]`.
    pub geofence_bounds: [f64; 4],
    /// Raw POI count returned by source ingestion.
    pub raw_poi_count: u64,
    /// Persisted POI count after geofence filtering.
    pub persisted_poi_count: u64,
}

/// Driving port for backend-owned OSM ingestion behaviour.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait OsmIngestionCommand: Send + Sync {
    /// Execute one ingestion run with deterministic rerun behaviour.
    async fn ingest(&self, request: OsmIngestionRequest) -> Result<OsmIngestionOutcome, Error>;
}

/// Fixture command implementation for tests without real ingestion.
#[derive(Debug, Clone, Copy, Default)]
pub struct FixtureOsmIngestionCommand;

#[async_trait]
impl OsmIngestionCommand for FixtureOsmIngestionCommand {
    async fn ingest(&self, request: OsmIngestionRequest) -> Result<OsmIngestionOutcome, Error> {
        Ok(OsmIngestionOutcome {
            status: OsmIngestionStatus::Executed,
            source_url: request.source_url,
            geofence_id: request.geofence_id,
            input_digest: request.input_digest,
            imported_at: Utc::now(),
            geofence_bounds: request.geofence_bounds,
            raw_poi_count: 0,
            persisted_poi_count: 0,
        })
    }
}
