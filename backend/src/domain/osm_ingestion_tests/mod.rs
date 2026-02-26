//! Shared test fixtures and module wiring for OSM ingestion unit tests.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::{DateTime, Local, TimeZone, Utc};
use mockable::Clock;

use super::{
    GeofenceBounds, RELATION_ID_PREFIX, WAY_ID_PREFIX, decode_element_id, validate_request,
};
use crate::domain::OsmIngestionCommandService;
use crate::domain::ports::{
    MockOsmIngestionProvenanceRepository, MockOsmSourceRepository, OsmIngestionProvenanceRecord,
    OsmIngestionRequest, OsmPoiIngestionRecord, OsmSourcePoi,
};

pub(super) const INPUT_DIGEST: &str =
    "2e7d2c03a9507ae265ecf5b5356885a53393a2029f7c98f0f8f9f8f2a5f1f7c6";
pub(super) const SOURCE_URL: &str = "https://example.test/launch.osm.pbf";
pub(super) const GEOFENCE_BOUNDS: [f64; 4] = [-3.30, 55.90, -3.10, 56.00];

pub(super) fn fixture_timestamp() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 2, 24, 10, 30, 0)
        .single()
        .expect("valid fixture timestamp")
}

struct FixtureClock {
    utc_now: DateTime<Utc>,
}

impl Clock for FixtureClock {
    fn local(&self) -> DateTime<Local> {
        self.utc_now.with_timezone(&Local)
    }

    fn utc(&self) -> DateTime<Utc> {
        self.utc_now
    }
}

pub(super) fn fixture_clock() -> Arc<dyn Clock> {
    Arc::new(FixtureClock {
        utc_now: fixture_timestamp(),
    })
}

pub(super) fn make_source_poi(
    encoded_element_id: u64,
    longitude: f64,
    latitude: f64,
) -> OsmSourcePoi {
    let tags = BTreeMap::from([("name".to_owned(), "Fixture POI".to_owned())]);
    OsmSourcePoi {
        encoded_element_id,
        longitude,
        latitude,
        tags,
    }
}

pub(super) fn request() -> OsmIngestionRequest {
    OsmIngestionRequest {
        osm_pbf_path: PathBuf::from("fixtures/launch.osm.pbf"),
        source_url: SOURCE_URL.to_owned(),
        geofence_id: "launch-a".to_owned(),
        geofence_bounds: GEOFENCE_BOUNDS,
        input_digest: INPUT_DIGEST.to_owned(),
    }
}

pub(super) fn make_service(
    source_repo: MockOsmSourceRepository,
    provenance_repo: MockOsmIngestionProvenanceRepository,
    clock: Arc<dyn Clock>,
) -> OsmIngestionCommandService<MockOsmSourceRepository, MockOsmIngestionProvenanceRepository> {
    OsmIngestionCommandService::new(Arc::new(source_repo), Arc::new(provenance_repo), clock)
}

mod decode_element_id;
mod ingest_behaviour;
mod request_validation;
