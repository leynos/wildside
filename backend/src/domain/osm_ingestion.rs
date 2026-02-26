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
    OsmIngestionProvenanceRepository, OsmIngestionRequest, OsmIngestionStatus,
    OsmPoiIngestionRecord, OsmSourcePoi, OsmSourceRepository,
};

#[path = "osm_ingestion_mapping.rs"]
mod mapping;
#[path = "osm_ingestion_service_helpers.rs"]
mod service_helpers;
#[path = "osm_ingestion_validation.rs"]
mod validation;

const WAY_ID_PREFIX: u64 = 1 << 62;
const RELATION_ID_PREFIX: u64 = 1 << 63;
const TYPE_ID_MASK: u64 = (1 << 62) - 1;

/// Validated geofence bounds in `[min_lng, min_lat, max_lng, max_lat]` order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GeofenceBounds {
    inner: [f64; 4],
}

impl GeofenceBounds {
    /// Construct validated bounds from explicit coordinate values.
    /// ```
    /// use backend::domain::osm_ingestion::GeofenceBounds;
    /// let bounds = GeofenceBounds::new(-3.30, 55.90, -3.10, 56.00).expect("valid bounds");
    /// assert_eq!(bounds.as_array(), [-3.30, 55.90, -3.10, 56.00]); // Ordered bounds persist.
    /// ```
    pub fn new(min_lng: f64, min_lat: f64, max_lng: f64, max_lat: f64) -> Result<Self, Error> {
        validation::validate_bounds([min_lng, min_lat, max_lng, max_lat])?;
        Ok(Self {
            inner: [min_lng, min_lat, max_lng, max_lat],
        })
    }

    /// Return whether a point lies within this geofence.
    /// ```
    /// use backend::domain::osm_ingestion::{Coordinate, GeofenceBounds};
    /// let bounds = GeofenceBounds::new(-3.30, 55.90, -3.10, 56.00).expect("valid bounds");
    /// let coordinate = Coordinate::new(-3.10, 56.00).expect("valid coordinate");
    /// assert!(bounds.contains(&coordinate)); // Boundary points are inside.
    /// ```
    pub fn contains(&self, coordinate: &Coordinate) -> bool {
        let [min_lng, min_lat, max_lng, max_lat] = self.inner;
        coordinate.longitude() >= min_lng
            && coordinate.longitude() <= max_lng
            && coordinate.latitude() >= min_lat
            && coordinate.latitude() <= max_lat
    }

    /// Expose bounds as a primitive array for port contracts.
    /// ```
    /// use backend::domain::osm_ingestion::GeofenceBounds;
    /// let bounds = GeofenceBounds::new(-3.30, 55.90, -3.10, 56.00).expect("valid bounds");
    /// assert_eq!(bounds.as_array(), [-3.30, 55.90, -3.10, 56.00]); // Adapter-facing format.
    /// ```
    pub fn as_array(&self) -> [f64; 4] {
        self.inner
    }
}

/// Validated SHA-256 input digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputDigest {
    digest: String,
}

impl InputDigest {
    /// Construct a validated input digest.
    /// ```
    /// use backend::domain::osm_ingestion::InputDigest;
    /// let digest = InputDigest::new("a".repeat(64)).expect("valid digest");
    /// assert_eq!(digest.as_str().len(), 64); // SHA-256 hex digest length.
    /// ```
    pub fn new(digest: String) -> Result<Self, Error> {
        if !validation::is_valid_digest(&digest) {
            return Err(Error::invalid_request(
                "inputDigest must be a 64-character lowercase hexadecimal SHA-256 digest",
            ));
        }
        Ok(Self { digest })
    }

    /// Borrow the underlying digest string.
    /// ```
    /// use backend::domain::osm_ingestion::InputDigest;
    /// let digest = InputDigest::new("a".repeat(64)).expect("valid digest");
    /// assert!(digest.as_str().starts_with('a')); // Access canonical digest bytes.
    /// ```
    pub fn as_str(&self) -> &str {
        &self.digest
    }
}

/// Validated geofence identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeofenceId(String);

impl GeofenceId {
    /// Construct a validated geofence identifier.
    /// ```
    /// use backend::domain::osm_ingestion::GeofenceId;
    /// let geofence_id = GeofenceId::new("  launch-a  ".to_owned()).expect("valid id");
    /// assert_eq!(geofence_id.as_str(), "launch-a"); // Surrounding whitespace is trimmed.
    /// ```
    pub fn new(id: String) -> Result<Self, Error> {
        let trimmed = id.trim();
        if trimmed.is_empty() {
            return Err(Error::invalid_request("geofenceId must not be empty"));
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Borrow the identifier string.
    /// ```
    /// use backend::domain::osm_ingestion::GeofenceId;
    /// let geofence_id = GeofenceId::new("launch-a".to_owned()).expect("valid id");
    /// assert_eq!(geofence_id.as_str(), "launch-a"); // Borrowed without allocation.
    /// ```
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Validated source URL used for provenance records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceUrl(String);

impl SourceUrl {
    /// Construct a validated source URL.
    /// ```
    /// use backend::domain::osm_ingestion::SourceUrl;
    /// let source_url = SourceUrl::new("https://example.test/launch.osm.pbf".to_owned()).expect("valid source URL");
    /// assert_eq!(source_url.as_str(), "https://example.test/launch.osm.pbf"); // URL is retained.
    /// ```
    pub fn new(url: String) -> Result<Self, Error> {
        let trimmed = url.trim();
        if trimmed.is_empty() {
            return Err(Error::invalid_request("sourceUrl must not be empty"));
        }
        if Url::parse(trimmed).is_err() {
            return Err(Error::invalid_request("sourceUrl must be a valid URL"));
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Borrow the URL string.
    /// ```
    /// use backend::domain::osm_ingestion::SourceUrl;
    /// let source_url = SourceUrl::new("https://example.test/launch.osm.pbf".to_owned()).expect("valid source URL");
    /// assert!(source_url.as_str().starts_with("https://")); // Borrow validated URL.
    /// ```
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Validated geographic coordinate (longitude, latitude).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coordinate {
    longitude: f64,
    latitude: f64,
}

impl Coordinate {
    /// Construct a validated coordinate.
    /// ```
    /// use backend::domain::osm_ingestion::Coordinate;
    /// let coordinate = Coordinate::new(-3.20, 55.95).expect("valid coordinate");
    /// assert_eq!(coordinate.longitude(), -3.20); // Longitude is retained.
    /// ```
    pub fn new(longitude: f64, latitude: f64) -> Result<Self, Error> {
        if !validation::valid_longitude(longitude) {
            return Err(Error::invalid_request(
                "longitude must be finite and within [-180, 180]",
            ));
        }
        if !validation::valid_latitude(latitude) {
            return Err(Error::invalid_request(
                "latitude must be finite and within [-90, 90]",
            ));
        }
        Ok(Self {
            longitude,
            latitude,
        })
    }

    /// Borrow the validated longitude.
    /// ```
    /// use backend::domain::osm_ingestion::Coordinate;
    /// let coordinate = Coordinate::new(-3.20, 55.95).expect("valid coordinate");
    /// assert_eq!(coordinate.longitude(), -3.20); // Access longitude component.
    /// ```
    pub fn longitude(&self) -> f64 {
        self.longitude
    }

    /// Borrow the validated latitude.
    /// ```
    /// use backend::domain::osm_ingestion::Coordinate;
    /// let coordinate = Coordinate::new(-3.20, 55.95).expect("valid coordinate");
    /// assert_eq!(coordinate.latitude(), 55.95); // Access latitude component.
    /// ```
    pub fn latitude(&self) -> f64 {
        self.latitude
    }
}

#[derive(Debug, Clone)]
struct ValidatedOsmIngestionRequest {
    source_url: SourceUrl,
    geofence_id: GeofenceId,
    geofence_bounds: GeofenceBounds,
    input_digest: InputDigest,
}

/// Domain service implementing OSM ingestion command behaviour.
#[derive(Clone)]
pub struct OsmIngestionCommandService<S, R> {
    source_repo: Arc<S>,
    provenance_repo: Arc<R>,
    clock: Arc<dyn Clock>,
}

impl<S, R> OsmIngestionCommandService<S, R> {
    /// Create a new ingestion service.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    ///
    /// use backend::domain::OsmIngestionCommandService;
    /// use backend::domain::ports::{
    ///     FixtureOsmIngestionProvenanceRepository, FixtureOsmSourceRepository,
    /// };
    /// use mockable::DefaultClock;
    ///
    /// let svc = OsmIngestionCommandService::new(
    ///     Arc::new(FixtureOsmSourceRepository),
    ///     Arc::new(FixtureOsmIngestionProvenanceRepository),
    ///     Arc::new(DefaultClock),
    /// );
    /// let _ = svc;
    /// ```
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
        let validated_request = validate_request(&request)?;

        if let Some(existing) = self.check_for_existing_rerun(&validated_request).await? {
            return Ok(mapping::to_outcome(OsmIngestionStatus::Replayed, existing));
        }

        let (filtered_records, raw_poi_count, filtered_poi_count) = self
            .ingest_and_filter_pois(&request, &validated_request)
            .await?;

        let provenance = OsmIngestionProvenanceRecord {
            geofence_id: validated_request.geofence_id.as_str().to_owned(),
            source_url: validated_request.source_url.as_str().to_owned(),
            input_digest: validated_request.input_digest.as_str().to_owned(),
            imported_at: self.clock.utc(),
            geofence_bounds: validated_request.geofence_bounds.as_array(),
            raw_poi_count,
            filtered_poi_count,
        };

        if let Some(existing) = self
            .persist_with_conflict_handling(&provenance, &filtered_records, &validated_request)
            .await?
        {
            return Ok(mapping::to_outcome(OsmIngestionStatus::Replayed, existing));
        }

        Ok(mapping::to_outcome(
            OsmIngestionStatus::Executed,
            provenance,
        ))
    }
}

fn validate_request(request: &OsmIngestionRequest) -> Result<ValidatedOsmIngestionRequest, Error> {
    let source_url = SourceUrl::new(request.source_url.clone())?;
    let geofence_id = GeofenceId::new(request.geofence_id.clone())?;
    let input_digest = InputDigest::new(request.input_digest.clone())?;
    let [min_lng, min_lat, max_lng, max_lat] = request.geofence_bounds;
    let geofence_bounds = GeofenceBounds::new(min_lng, min_lat, max_lng, max_lat)?;

    Ok(ValidatedOsmIngestionRequest {
        source_url,
        geofence_id,
        geofence_bounds,
        input_digest,
    })
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

/// Decode a packed OSM element identifier into `(element_type, element_id)`.
///
/// `encoded_id` uses a bit-prefix scheme where:
/// - bit 63 (`RELATION_ID_PREFIX`) marks a `"relation"`;
/// - bit 62 (`WAY_ID_PREFIX`) marks a `"way"`;
/// - when neither prefix is set, the value is a `"node"` identifier.
///
/// `TYPE_ID_MASK` extracts the raw identifier bits for prefixed values. The raw
/// id is then converted to `i64`, returning an invalid-request error when it
/// exceeds `i64` range.
fn decode_element_id(encoded_id: u64) -> Result<(String, i64), Error> {
    let (element_type, raw_id) = classify_element_prefix(encoded_id);

    let element_id = i64::try_from(raw_id)
        .map_err(|_| Error::invalid_request("decoded OSM element identifier exceeds i64 range"))?;
    Ok((element_type.to_owned(), element_id))
}

/// Classifies `encoded_id` into `(type, raw_id)` where type is `node`/`way`/`relation` and `raw_id` has type bits masked off.
fn classify_element_prefix(encoded_id: u64) -> (&'static str, u64) {
    if encoded_id & RELATION_ID_PREFIX != 0 {
        ("relation", encoded_id & TYPE_ID_MASK)
    } else if encoded_id & WAY_ID_PREFIX != 0 {
        ("way", encoded_id & TYPE_ID_MASK)
    } else {
        ("node", encoded_id)
    }
}

#[cfg(test)]
#[path = "osm_ingestion_tests/mod.rs"]
mod tests;
