//! Outbound adapter for OSM extraction using `wildside-data`.

use std::collections::BTreeMap;
use std::path::Path;

use async_trait::async_trait;

use crate::domain::ports::{
    OsmSourcePoi, OsmSourceReport, OsmSourceRepository, OsmSourceRepositoryError,
};

/// `wildside-data` backed source adapter.
#[derive(Debug, Clone, Copy, Default)]
pub struct WildsideDataOsmSourceRepository;

#[async_trait]
impl OsmSourceRepository for WildsideDataOsmSourceRepository {
    async fn ingest_osm_pbf(
        &self,
        path: &Path,
    ) -> Result<OsmSourceReport, OsmSourceRepositoryError> {
        let report = wildside_data::ingest_osm_pbf_report(path).map_err(map_ingest_error)?;
        let pois = report
            .pois
            .into_iter()
            .map(|poi| OsmSourcePoi {
                encoded_element_id: poi.id,
                longitude: poi.location.x,
                latitude: poi.location.y,
                tags: poi.tags.into_iter().collect::<BTreeMap<_, _>>(),
            })
            .collect();
        Ok(OsmSourceReport { pois })
    }
}

fn map_ingest_error(error: wildside_data::OsmIngestError) -> OsmSourceRepositoryError {
    match error {
        wildside_data::OsmIngestError::Open { source, path } => {
            OsmSourceRepositoryError::read(format!("{source} ({})", path.display()))
        }
        wildside_data::OsmIngestError::Decode { source, path } => {
            OsmSourceRepositoryError::decode(format!("{source} ({})", path.display()))
        }
    }
}
