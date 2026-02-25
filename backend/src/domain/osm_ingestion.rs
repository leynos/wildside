//! OSM ingestion orchestration service.
//!
//! This service owns backend-specific ingestion behaviour:
//! - geofence filtering;
//! - provenance persistence;
//! - deterministic reruns keyed by geofence and input digest.

use std::sync::Arc;

use async_trait::async_trait;
use mockable::Clock;
use url::Url;

use crate::domain::Error;
use crate::domain::ports::{
    OsmIngestionCommand, OsmIngestionOutcome, OsmIngestionProvenanceRecord,
    OsmIngestionProvenanceRepository, OsmIngestionProvenanceRepositoryError, OsmIngestionRequest,
    OsmIngestionStatus, OsmPoiIngestionRecord, OsmSourcePoi, OsmSourceRepository,
    OsmSourceRepositoryError,
};

const WAY_ID_PREFIX: u64 = 1 << 62;
const RELATION_ID_PREFIX: u64 = 1 << 63;
const TYPE_ID_MASK: u64 = (1 << 62) - 1;

/// Domain service implementing OSM ingestion command behaviour.
#[derive(Clone)]
pub struct OsmIngestionCommandService<S, R> {
    source_repo: Arc<S>,
    provenance_repo: Arc<R>,
    clock: Arc<dyn Clock>,
}

impl<S, R> OsmIngestionCommandService<S, R> {
    /// Create a new ingestion service.
    pub fn new(source_repo: Arc<S>, provenance_repo: Arc<R>, clock: Arc<dyn Clock>) -> Self {
        Self {
            source_repo,
            provenance_repo,
            clock,
        }
    }
}

#[async_trait]
impl<S, R> OsmIngestionCommand for OsmIngestionCommandService<S, R>
where
    S: OsmSourceRepository,
    R: OsmIngestionProvenanceRepository,
{
    async fn ingest(&self, request: OsmIngestionRequest) -> Result<OsmIngestionOutcome, Error> {
        validate_request(&request)?;

        if let Some(existing) = self
            .provenance_repo
            .find_by_rerun_key(&request.geofence_id, &request.input_digest)
            .await
            .map_err(map_provenance_error)?
        {
            return Ok(to_outcome(OsmIngestionStatus::Replayed, existing));
        }

        let source_report = self
            .source_repo
            .ingest_osm_pbf(&request.osm_pbf_path)
            .await
            .map_err(map_source_error)?;
        let raw_poi_count = u64::try_from(source_report.pois.len())
            .map_err(|_| Error::internal("raw POI count exceeds supported range"))?;

        let filtered_records = source_report
            .pois
            .into_iter()
            .filter(|poi| geofence_contains(request.geofence_bounds, poi.longitude, poi.latitude))
            .map(to_poi_record)
            .collect::<Result<Vec<_>, _>>()?;
        let filtered_poi_count = u64::try_from(filtered_records.len())
            .map_err(|_| Error::internal("filtered POI count exceeds supported range"))?;

        let provenance = OsmIngestionProvenanceRecord {
            geofence_id: request.geofence_id.clone(),
            source_url: request.source_url.clone(),
            input_digest: request.input_digest.clone(),
            imported_at: self.clock.utc(),
            geofence_bounds: request.geofence_bounds,
            raw_poi_count,
            filtered_poi_count,
        };

        match self
            .provenance_repo
            .persist_ingestion(&provenance, &filtered_records)
            .await
        {
            Ok(()) => {}
            Err(OsmIngestionProvenanceRepositoryError::Conflict { .. }) => {
                let existing = self
                    .provenance_repo
                    .find_by_rerun_key(&request.geofence_id, &request.input_digest)
                    .await
                    .map_err(map_provenance_error)?
                    .ok_or_else(|| {
                        Error::service_unavailable(
                            "ingestion provenance conflict occurred but rerun key was not found",
                        )
                    })?;
                return Ok(to_outcome(OsmIngestionStatus::Replayed, existing));
            }
            Err(error) => return Err(map_provenance_error(error)),
        }

        Ok(to_outcome(OsmIngestionStatus::Executed, provenance))
    }
}

fn validate_request(request: &OsmIngestionRequest) -> Result<(), Error> {
    if request.source_url.trim().is_empty() {
        return Err(Error::invalid_request("sourceUrl must not be empty"));
    }
    if Url::parse(&request.source_url).is_err() {
        return Err(Error::invalid_request("sourceUrl must be a valid URL"));
    }
    if request.geofence_id.trim().is_empty() {
        return Err(Error::invalid_request("geofenceId must not be empty"));
    }
    if !is_valid_digest(&request.input_digest) {
        return Err(Error::invalid_request(
            "inputDigest must be a 64-character lowercase hexadecimal SHA-256 digest",
        ));
    }

    validate_bounds(request.geofence_bounds)
}

fn is_valid_digest(digest: &str) -> bool {
    digest.len() == 64
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn validate_bounds(bounds: [f64; 4]) -> Result<(), Error> {
    let [min_lng, min_lat, max_lng, max_lat] = bounds;
    validate_longitude_bounds(min_lng, max_lng)?;
    validate_latitude_bounds(min_lat, max_lat)?;
    validate_bounds_ordering(min_lng, min_lat, max_lng, max_lat)?;
    Ok(())
}

fn validate_longitude_bounds(min_lng: f64, max_lng: f64) -> Result<(), Error> {
    if !(valid_longitude(min_lng) && valid_longitude(max_lng)) {
        return Err(Error::invalid_request(
            "geofence longitude values must be finite and within [-180, 180]",
        ));
    }

    Ok(())
}

fn validate_latitude_bounds(min_lat: f64, max_lat: f64) -> Result<(), Error> {
    if !(valid_latitude(min_lat) && valid_latitude(max_lat)) {
        return Err(Error::invalid_request(
            "geofence latitude values must be finite and within [-90, 90]",
        ));
    }

    Ok(())
}

fn validate_bounds_ordering(
    min_lng: f64,
    min_lat: f64,
    max_lng: f64,
    max_lat: f64,
) -> Result<(), Error> {
    if min_lng <= max_lng && min_lat <= max_lat {
        return Ok(());
    }

    Err(Error::invalid_request(
        "geofenceBounds must be ordered as [minLng, minLat, maxLng, maxLat]",
    ))
}

#[rustfmt::skip]
fn valid_longitude(value: f64) -> bool { value.is_finite() && (-180.0..=180.0).contains(&value) }

#[rustfmt::skip]
fn valid_latitude(value: f64) -> bool { value.is_finite() && (-90.0..=90.0).contains(&value) }

fn geofence_contains(bounds: [f64; 4], longitude: f64, latitude: f64) -> bool {
    let [min_lng, min_lat, max_lng, max_lat] = bounds;
    longitude.is_finite()
        && latitude.is_finite()
        && longitude >= min_lng
        && longitude <= max_lng
        && latitude >= min_lat
        && latitude <= max_lat
}

fn to_poi_record(source_poi: OsmSourcePoi) -> Result<OsmPoiIngestionRecord, Error> {
    let (element_type, element_id) = decode_element_id(source_poi.encoded_element_id)?;

    Ok(OsmPoiIngestionRecord {
        element_type,
        element_id,
        longitude: source_poi.longitude,
        latitude: source_poi.latitude,
        tags: source_poi.tags,
    })
}

fn decode_element_id(encoded_id: u64) -> Result<(String, i64), Error> {
    let (element_type, raw_id) = if encoded_id & RELATION_ID_PREFIX != 0 {
        ("relation", encoded_id & TYPE_ID_MASK)
    } else if encoded_id & WAY_ID_PREFIX != 0 {
        ("way", encoded_id & TYPE_ID_MASK)
    } else {
        ("node", encoded_id)
    };

    let element_id = i64::try_from(raw_id)
        .map_err(|_| Error::invalid_request("decoded OSM element identifier exceeds i64 range"))?;
    Ok((element_type.to_owned(), element_id))
}

fn to_outcome(
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

fn map_source_error(error: OsmSourceRepositoryError) -> Error {
    match error {
        OsmSourceRepositoryError::Read { message }
        | OsmSourceRepositoryError::Decode { message } => {
            Error::service_unavailable(format!("failed to ingest OSM source: {message}"))
        }
    }
}

fn map_provenance_error(error: OsmIngestionProvenanceRepositoryError) -> Error {
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

#[cfg(test)]
#[path = "osm_ingestion_tests/mod.rs"]
mod tests;
