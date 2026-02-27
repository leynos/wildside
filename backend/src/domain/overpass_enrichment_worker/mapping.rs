//! Mapping helpers for worker outcomes, failures, and source payloads.

use crate::domain::Error;
use crate::domain::overpass_enrichment_worker::policy::QuotaDenyReason;
use crate::domain::ports::{
    EnrichmentJobFailureKind, OsmPoiIngestionRecord, OsmPoiRepositoryError,
    OverpassEnrichmentSourceError, OverpassPoi,
};

pub(super) fn map_overpass_poi(poi: OverpassPoi) -> OsmPoiIngestionRecord {
    OsmPoiIngestionRecord {
        element_type: poi.element_type,
        element_id: poi.element_id,
        longitude: poi.longitude,
        latitude: poi.latitude,
        tags: poi.tags,
    }
}

pub(super) fn map_quota_failure_kind(reason: QuotaDenyReason) -> EnrichmentJobFailureKind {
    match reason {
        QuotaDenyReason::RequestLimit => EnrichmentJobFailureKind::QuotaRequestLimit,
        QuotaDenyReason::TransferLimit => EnrichmentJobFailureKind::QuotaTransferLimit,
    }
}

pub(super) fn map_quota_error(reason: QuotaDenyReason) -> Error {
    match reason {
        QuotaDenyReason::RequestLimit => {
            Error::service_unavailable("daily Overpass request quota exhausted")
        }
        QuotaDenyReason::TransferLimit => {
            Error::service_unavailable("daily Overpass transfer quota exhausted")
        }
    }
}

pub(super) fn map_retry_exhausted_error(error: OverpassEnrichmentSourceError) -> Error {
    Error::service_unavailable(format!("overpass retries exhausted: {error}"))
}

pub(super) fn map_source_rejected_error(error: OverpassEnrichmentSourceError) -> Error {
    match error {
        OverpassEnrichmentSourceError::InvalidRequest { message } => {
            Error::invalid_request(format!("overpass request rejected: {message}"))
        }
        other => Error::internal(format!("overpass call failed: {other}")),
    }
}

pub(super) fn map_persistence_error(error: OsmPoiRepositoryError, attempts: u32) -> Error {
    match error {
        OsmPoiRepositoryError::Connection { message } => Error::service_unavailable(format!(
            "enrichment persistence unavailable after {attempts} attempts: {message}"
        )),
        OsmPoiRepositoryError::Query { message } => Error::internal(format!(
            "enrichment persistence failed after {attempts} attempts: {message}"
        )),
    }
}
