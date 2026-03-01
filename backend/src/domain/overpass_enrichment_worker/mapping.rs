//! Mapping helpers for worker outcomes, failures, and source payloads.

use crate::domain::Error;
use crate::domain::overpass_enrichment_worker::policy::QuotaDenyReason;
use crate::domain::ports::{
    EnrichmentJobFailureKind, EnrichmentProvenanceRepositoryError, OsmPoiIngestionRecord,
    OsmPoiRepositoryError, OverpassEnrichmentSourceError, OverpassPoi,
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

#[expect(
    clippy::too_many_arguments,
    reason = "Helper signature intentionally mirrors the required shared error-mapping inputs."
)]
fn map_repository_persistence_error(
    context: &str,
    attempts: u32,
    connection_message: String,
    query_message: String,
    is_connection_error: bool,
) -> Error {
    if is_connection_error {
        Error::service_unavailable(format!(
            "{context} unavailable after {attempts} attempts: {connection_message}"
        ))
    } else {
        Error::internal(format!(
            "{context} failed after {attempts} attempts: {query_message}"
        ))
    }
}

pub(super) fn map_persistence_error(error: OsmPoiRepositoryError, attempts: u32) -> Error {
    let (connection_message, query_message, is_connection_error) = match error {
        OsmPoiRepositoryError::Connection { message } => (message, String::new(), true),
        OsmPoiRepositoryError::Query { message } => (String::new(), message, false),
    };

    map_repository_persistence_error(
        "enrichment persistence",
        attempts,
        connection_message,
        query_message,
        is_connection_error,
    )
}

pub(super) fn map_provenance_persistence_error(
    error: EnrichmentProvenanceRepositoryError,
    attempts: u32,
) -> Error {
    let (connection_message, query_message, is_connection_error) = match error {
        EnrichmentProvenanceRepositoryError::Connection { message } => {
            (message, String::new(), true)
        }
        EnrichmentProvenanceRepositoryError::Query { message } => (String::new(), message, false),
    };

    map_repository_persistence_error(
        "enrichment provenance persistence",
        attempts,
        connection_message,
        query_message,
        is_connection_error,
    )
}
