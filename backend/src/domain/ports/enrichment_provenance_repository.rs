//! Driven port for enrichment provenance persistence and reporting.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use super::define_port_error;

/// Persisted provenance record for one successful enrichment source fetch.
#[derive(Debug, Clone, PartialEq)]
pub struct EnrichmentProvenanceRecord {
    /// Stable enrichment job identifier used for idempotent persistence.
    pub job_id: Uuid,
    /// Source URL used for the successful enrichment call.
    pub source_url: String,
    /// Timestamp when provenance was imported into backend persistence.
    pub imported_at: DateTime<Utc>,
    /// Bounding box used for the enrichment request `[min_lng, min_lat, max_lng, max_lat]`.
    pub bounding_box: [f64; 4],
}

/// Stable cursor for keyset pagination of enrichment provenance records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnrichmentProvenanceCursor {
    /// Imported timestamp component of the ordering key.
    pub imported_at: DateTime<Utc>,
    /// Tie-breaker component of the ordering key.
    pub id: Uuid,
}

impl EnrichmentProvenanceCursor {
    /// Construct a cursor from ordering key components.
    pub const fn new(imported_at: DateTime<Utc>, id: Uuid) -> Self {
        Self { imported_at, id }
    }

    /// Return cursor components as a tuple.
    pub const fn into_tuple(self) -> (DateTime<Utc>, Uuid) {
        (self.imported_at, self.id)
    }
}

/// Query parameters for listing recently imported enrichment provenance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListEnrichmentProvenanceRequest {
    /// Maximum rows to return.
    pub limit: usize,
    /// Optional exclusive composite cursor for `(imported_at, id)`.
    pub before: Option<EnrichmentProvenanceCursor>,
}

impl ListEnrichmentProvenanceRequest {
    /// Construct a request for listing recent enrichment provenance rows.
    pub fn new(limit: usize, before: Option<(DateTime<Utc>, Uuid)>) -> Self {
        Self {
            limit,
            before: before
                .map(|(imported_at, id)| EnrichmentProvenanceCursor::new(imported_at, id)),
        }
    }
}

/// Page of recent enrichment provenance records.
#[derive(Debug, Clone, PartialEq)]
pub struct ListEnrichmentProvenanceResponse {
    /// Newest-first records.
    pub records: Vec<EnrichmentProvenanceRecord>,
    /// Optional cursor for the next page.
    pub next_before: Option<EnrichmentProvenanceCursor>,
}

define_port_error! {
    /// Errors raised while reading or writing enrichment provenance rows.
    pub enum EnrichmentProvenanceRepositoryError {
        /// Repository connection could not be established.
        Connection { message: String } =>
            "enrichment provenance connection failed: {message}",
        /// Query or mutation failed during execution.
        Query { message: String } =>
            "enrichment provenance query failed: {message}",
    }
}

/// Port for persistence and reporting of enrichment provenance.
#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait EnrichmentProvenanceRepository: Send + Sync {
    /// Persist one enrichment provenance record.
    async fn persist(
        &self,
        record: &EnrichmentProvenanceRecord,
    ) -> Result<(), EnrichmentProvenanceRepositoryError>;

    /// List recent enrichment provenance records in deterministic order.
    async fn list_recent(
        &self,
        request: &ListEnrichmentProvenanceRequest,
    ) -> Result<ListEnrichmentProvenanceResponse, EnrichmentProvenanceRepositoryError>;
}

/// Fixture repository implementation for tests without persistence coupling.
#[derive(Debug, Clone, Copy, Default)]
pub struct FixtureEnrichmentProvenanceRepository;

#[async_trait]
impl EnrichmentProvenanceRepository for FixtureEnrichmentProvenanceRepository {
    async fn persist(
        &self,
        _record: &EnrichmentProvenanceRecord,
    ) -> Result<(), EnrichmentProvenanceRepositoryError> {
        Ok(())
    }

    async fn list_recent(
        &self,
        _request: &ListEnrichmentProvenanceRequest,
    ) -> Result<ListEnrichmentProvenanceResponse, EnrichmentProvenanceRepositoryError> {
        Ok(ListEnrichmentProvenanceResponse {
            records: Vec::new(),
            next_before: None,
        })
    }
}
