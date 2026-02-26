//! Internal service helpers for OSM ingestion orchestration sequencing.

use super::{
    Coordinate, OsmIngestionCommandService, ValidatedOsmIngestionRequest, mapping, to_poi_record,
};
use crate::domain::Error;
use crate::domain::ports::{
    OsmIngestionProvenanceRecord, OsmIngestionProvenanceRepository,
    OsmIngestionProvenanceRepositoryError, OsmIngestionRequest, OsmPoiIngestionRecord,
    OsmSourceRepository,
};

impl<S, R> OsmIngestionCommandService<S, R>
where
    S: OsmSourceRepository,
    R: OsmIngestionProvenanceRepository,
{
    pub(super) async fn check_for_existing_rerun(
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

    pub(super) async fn ingest_and_filter_pois(
        &self,
        request: &OsmIngestionRequest,
        validated_request: &ValidatedOsmIngestionRequest,
    ) -> Result<(Vec<OsmPoiIngestionRecord>, u64, u64), Error> {
        let source_report = self
            .source_repo
            .ingest_osm_pbf(&request.osm_pbf_path)
            .await
            .map_err(mapping::map_source_error)?;
        let raw_poi_count = u64::try_from(source_report.pois.len())
            .map_err(|_| Error::internal("raw POI count exceeds supported range"))?;
        let filtered_records = source_report
            .pois
            .into_iter()
            .filter(|poi| {
                Coordinate::new(poi.longitude, poi.latitude)
                    .map(|coord| validated_request.geofence_bounds.contains(&coord))
                    .unwrap_or(false)
            })
            .map(to_poi_record)
            .collect::<Result<Vec<_>, _>>()?;
        let filtered_poi_count = u64::try_from(filtered_records.len())
            .map_err(|_| Error::internal("filtered POI count exceeds supported range"))?;
        Ok((filtered_records, raw_poi_count, filtered_poi_count))
    }

    pub(super) async fn persist_with_conflict_handling(
        &self,
        provenance: &OsmIngestionProvenanceRecord,
        filtered_records: &[OsmPoiIngestionRecord],
        validated_request: &ValidatedOsmIngestionRequest,
    ) -> Result<Option<OsmIngestionProvenanceRecord>, Error> {
        match self
            .provenance_repo
            .persist_ingestion(provenance, filtered_records)
            .await
        {
            Ok(()) => Ok(None),
            Err(OsmIngestionProvenanceRepositoryError::Conflict { .. }) => {
                let existing = self
                    .provenance_repo
                    .find_by_rerun_key(
                        validated_request.geofence_id.as_str(),
                        validated_request.input_digest.as_str(),
                    )
                    .await
                    .map_err(mapping::map_provenance_error)?
                    .ok_or_else(|| {
                        Error::service_unavailable(
                            "ingestion provenance conflict occurred but rerun key was not found",
                        )
                    })?;
                Ok(Some(existing))
            }
            Err(error) => Err(mapping::map_provenance_error(error)),
        }
    }
}
