//! Test doubles for admin enrichment provenance reporting.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use backend::domain::ports::{
    EnrichmentProvenanceRecord, EnrichmentProvenanceRepository,
    EnrichmentProvenanceRepositoryError, ListEnrichmentProvenanceRequest,
    ListEnrichmentProvenanceResponse,
};

/// Configurable success or failure outcome for provenance list queries.
#[derive(Clone)]
pub(crate) enum EnrichmentProvenanceListResponse {
    Ok(ListEnrichmentProvenanceResponse),
    Err(EnrichmentProvenanceRepositoryError),
}

/// Recording double for [`EnrichmentProvenanceRepository`] query behaviour.
#[derive(Clone)]
pub(crate) struct RecordingEnrichmentProvenanceRepository {
    calls: Arc<Mutex<Vec<ListEnrichmentProvenanceRequest>>>,
    response: Arc<Mutex<EnrichmentProvenanceListResponse>>,
}

impl RecordingEnrichmentProvenanceRepository {
    pub(crate) fn new(response: EnrichmentProvenanceListResponse) -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            response: Arc::new(Mutex::new(response)),
        }
    }

    pub(crate) fn calls(&self) -> Vec<ListEnrichmentProvenanceRequest> {
        self.calls
            .lock()
            .expect("enrichment provenance calls lock")
            .clone()
    }

    pub(crate) fn set_response(&self, response: EnrichmentProvenanceListResponse) {
        *self
            .response
            .lock()
            .expect("enrichment provenance response lock") = response;
    }
}

#[async_trait]
impl EnrichmentProvenanceRepository for RecordingEnrichmentProvenanceRepository {
    async fn persist(
        &self,
        _record: &EnrichmentProvenanceRecord,
    ) -> Result<(), EnrichmentProvenanceRepositoryError> {
        Ok(())
    }

    async fn list_recent(
        &self,
        request: &ListEnrichmentProvenanceRequest,
    ) -> Result<ListEnrichmentProvenanceResponse, EnrichmentProvenanceRepositoryError> {
        self.calls
            .lock()
            .expect("enrichment provenance calls lock")
            .push(*request);

        match self
            .response
            .lock()
            .expect("enrichment provenance response lock")
            .clone()
        {
            EnrichmentProvenanceListResponse::Ok(response) => Ok(response),
            EnrichmentProvenanceListResponse::Err(error) => Err(error),
        }
    }
}
