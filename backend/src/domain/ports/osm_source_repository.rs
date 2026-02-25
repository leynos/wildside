//! Driven port for extracting POIs from OSM PBF inputs.
//!
//! This port isolates data-source specific ingestion mechanics so domain
//! orchestration can remain independent from concrete parser implementations.

use std::collections::BTreeMap;
use std::path::Path;

use async_trait::async_trait;

use super::define_port_error;

/// A POI extracted from an OSM source.
#[derive(Debug, Clone, PartialEq)]
pub struct OsmSourcePoi {
    /// Encoded OSM identifier with type prefix bits.
    pub encoded_element_id: u64,
    /// Longitude in WGS84.
    pub longitude: f64,
    /// Latitude in WGS84.
    pub latitude: f64,
    /// Raw OSM tags.
    pub tags: BTreeMap<String, String>,
}

/// Source ingestion result used by domain orchestration.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct OsmSourceReport {
    /// Extracted points of interest.
    pub pois: Vec<OsmSourcePoi>,
}

define_port_error! {
    /// Errors raised while reading or decoding OSM source data.
    pub enum OsmSourceRepositoryError {
        /// The source could not be read from disk.
        Read { message: String } =>
            "osm source read failed: {message}",
        /// The source could not be decoded into POI records.
        Decode { message: String } =>
            "osm source decode failed: {message}",
    }
}

/// Port for obtaining POI snapshots from an OSM PBF input.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait OsmSourceRepository: Send + Sync {
    /// Parse the given OSM PBF file into an in-memory POI report.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// use std::path::Path;
    ///
    /// use backend::domain::ports::{FixtureOsmSourceRepository, OsmSourceRepository};
    ///
    /// let repository = FixtureOsmSourceRepository::default();
    /// let report = repository
    ///     .ingest_osm_pbf(Path::new("fixtures/launch.osm.pbf"))
    ///     .await?;
    /// assert!(report.pois.is_empty());
    /// # Ok::<(), backend::domain::ports::OsmSourceRepositoryError>(())
    /// ```
    async fn ingest_osm_pbf(
        &self,
        path: &Path,
    ) -> Result<OsmSourceReport, OsmSourceRepositoryError>;
}

/// Fixture source implementation returning no POIs.
#[derive(Debug, Clone, Copy, Default)]
pub struct FixtureOsmSourceRepository;

#[async_trait]
impl OsmSourceRepository for FixtureOsmSourceRepository {
    async fn ingest_osm_pbf(
        &self,
        _path: &Path,
    ) -> Result<OsmSourceReport, OsmSourceRepositoryError> {
        Ok(OsmSourceReport::default())
    }
}
