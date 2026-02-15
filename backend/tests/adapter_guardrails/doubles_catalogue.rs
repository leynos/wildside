//! Test doubles for catalogue and descriptor read ports.

use super::recording_double_macro::recording_double;
use backend::domain::ports::{
    CatalogueRepository, CatalogueRepositoryError, DescriptorRepository, DescriptorRepositoryError,
    DescriptorSnapshot, ExploreCatalogueSnapshot,
};

recording_double! {
    /// Configurable success or failure outcome for RecordingCatalogueRepository.
    pub(crate) enum CatalogueQueryResponse {
        Ok(ExploreCatalogueSnapshot),
        Err(CatalogueRepositoryError),
    }

    pub(crate) struct RecordingCatalogueRepository {
        calls: (),
        trait: CatalogueRepository,
        method: explore_snapshot(&self) -> Result<ExploreCatalogueSnapshot, CatalogueRepositoryError>,
        record: (),
        calls_lock: "catalogue calls lock",
        response_lock: "catalogue response lock",
    }
}

recording_double! {
    /// Configurable success or failure outcome for RecordingDescriptorRepository.
    pub(crate) enum DescriptorQueryResponse {
        Ok(DescriptorSnapshot),
        Err(DescriptorRepositoryError),
    }

    pub(crate) struct RecordingDescriptorRepository {
        calls: (),
        trait: DescriptorRepository,
        method: descriptor_snapshot(&self) -> Result<DescriptorSnapshot, DescriptorRepositoryError>,
        record: (),
        calls_lock: "descriptor calls lock",
        response_lock: "descriptor response lock",
    }
}
