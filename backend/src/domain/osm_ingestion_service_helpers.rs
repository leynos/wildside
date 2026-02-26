//! Internal service helpers for OSM ingestion orchestration sequencing.

use std::path::Path;

use super::{
    Coordinate, GeofenceBounds, OsmIngestionCommandService, ValidatedOsmIngestionRequest, mapping,
    to_poi_record,
};
use crate::domain::Error;
use crate::domain::ports::{
    OsmIngestionProvenanceRecord, OsmIngestionProvenanceRepository,
    OsmIngestionProvenanceRepositoryError, OsmIngestionStatus, OsmPoiIngestionRecord,
    OsmSourceReport, OsmSourceRepository,
};

impl<S, R> OsmIngestionCommandService<S, R>
where
    S: OsmSourceRepository,
    R: OsmIngestionProvenanceRepository,
{
    pub(super) async fn lookup_rerun(
        &self,
        validated_request: &ValidatedOsmIngestionRequest,
    ) -> Result<Option<OsmIngestionProvenanceRecord>, Error> {
        self.provenance_repo
            .find_by_rerun_key(
                validated_request.geofence_id.as_str(),
                validated_request.input_digest.as_str(),
            )
            .await
            .map_err(mapping::map_provenance_error)
    }

    pub(super) async fn load_source(
        &self,
        osm_pbf_path: &Path,
    ) -> Result<(OsmSourceReport, u64), Error> {
        let source_report = self
            .source_repo
            .ingest_osm_pbf(osm_pbf_path)
            .await
            .map_err(mapping::map_source_error)?;
        let raw_poi_count = u64::try_from(source_report.pois.len())
            .map_err(|_| Error::internal("raw POI count exceeds supported range"))?;
        Ok((source_report, raw_poi_count))
    }

    pub(super) fn filter_to_poi_records(
        &self,
        source_report: OsmSourceReport,
        geofence_bounds: &GeofenceBounds,
    ) -> Result<(Vec<OsmPoiIngestionRecord>, u64), Error> {
        let filtered_records = source_report
            .pois
            .into_iter()
            .filter(|poi| {
                Coordinate::new(poi.longitude, poi.latitude)
                    .map(|coordinate| geofence_bounds.contains(&coordinate))
                    .unwrap_or(false)
            })
            .map(to_poi_record)
            .collect::<Result<Vec<_>, _>>()?;
        let filtered_poi_count = u64::try_from(filtered_records.len())
            .map_err(|_| Error::internal("filtered POI count exceeds supported range"))?;
        Ok((filtered_records, filtered_poi_count))
    }

    pub(super) fn build_provenance(
        &self,
        validated_request: &ValidatedOsmIngestionRequest,
        raw_poi_count: u64,
        filtered_poi_count: u64,
    ) -> OsmIngestionProvenanceRecord {
        OsmIngestionProvenanceRecord {
            geofence_id: validated_request.geofence_id.as_str().to_owned(),
            source_url: validated_request.source_url.as_str().to_owned(),
            input_digest: validated_request.input_digest.as_str().to_owned(),
            imported_at: self.clock.utc(),
            geofence_bounds: validated_request.geofence_bounds.as_array(),
            raw_poi_count,
            filtered_poi_count,
        }
    }

    pub(super) async fn persist_or_replay(
        &self,
        provenance: OsmIngestionProvenanceRecord,
        filtered_records: &[OsmPoiIngestionRecord],
        validated_request: &ValidatedOsmIngestionRequest,
    ) -> Result<(OsmIngestionStatus, OsmIngestionProvenanceRecord), Error> {
        match self
            .provenance_repo
            .persist_ingestion(&provenance, filtered_records)
            .await
        {
            Ok(()) => Ok((OsmIngestionStatus::Executed, provenance)),
            Err(OsmIngestionProvenanceRepositoryError::Conflict { .. }) => {
                let existing = self.lookup_rerun(validated_request).await?.ok_or_else(|| {
                    Error::service_unavailable(
                        "ingestion provenance conflict occurred but rerun key was not found",
                    )
                })?;
                Ok((OsmIngestionStatus::Replayed, existing))
            }
            Err(error) => Err(mapping::map_provenance_error(error)),
        }
    }
}
