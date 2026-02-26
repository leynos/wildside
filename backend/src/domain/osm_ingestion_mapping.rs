//! Internal outcome and error mapping helpers for OSM ingestion service flow.

use crate::domain::Error;
use crate::domain::ports::{
    OsmIngestionOutcome, OsmIngestionProvenanceRecord, OsmIngestionProvenanceRepositoryError,
    OsmIngestionStatus, OsmSourceRepositoryError,
};

pub(super) fn to_outcome(
    status: OsmIngestionStatus,
    record: OsmIngestionProvenanceRecord,
) -> OsmIngestionOutcome {
    OsmIngestionOutcome {
        status,
        source_url: record.source_url,
        geofence_id: record.geofence_id,
        input_digest: record.input_digest,
        imported_at: record.imported_at,
        geofence_bounds: record.geofence_bounds,
        raw_poi_count: record.raw_poi_count,
        persisted_poi_count: record.filtered_poi_count,
    }
}

pub(super) fn map_source_error(error: OsmSourceRepositoryError) -> Error {
    match error {
        OsmSourceRepositoryError::Read { message }
        | OsmSourceRepositoryError::Decode { message } => {
            Error::service_unavailable(format!("failed to ingest OSM source: {message}"))
        }
    }
}

pub(super) fn map_provenance_error(error: OsmIngestionProvenanceRepositoryError) -> Error {
    match error {
        OsmIngestionProvenanceRepositoryError::Connection { message }
        | OsmIngestionProvenanceRepositoryError::Query { message } => {
            Error::service_unavailable(format!("failed to persist ingestion provenance: {message}"))
        }
        OsmIngestionProvenanceRepositoryError::Conflict { message } => {
            Error::conflict(format!("ingestion rerun key conflict: {message}"))
        }
    }
}
