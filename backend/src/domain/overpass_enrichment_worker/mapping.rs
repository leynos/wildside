//! Mapping helpers for worker outcomes, failures, and source payloads.

use crate::domain::Error;
use crate::domain::enrichment_provenance_error_mapping::map_enrichment_provenance_repository_error;
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

enum RepositoryPersistenceErrorKind {
    Connection { message: String },
    Query { message: String },
}

struct RepositoryErrorContext<'a> {
    context: &'a str,
    attempts: u32,
    kind: RepositoryPersistenceErrorKind,
}

fn map_repository_persistence_error(context: RepositoryErrorContext<'_>) -> Error {
    match context.kind {
        RepositoryPersistenceErrorKind::Connection { message } => {
            Error::service_unavailable(format!(
                "{} unavailable after {} attempts: {}",
                context.context, context.attempts, message
            ))
        }
        RepositoryPersistenceErrorKind::Query { message } => Error::internal(format!(
            "{} failed after {} attempts: {}",
            context.context, context.attempts, message
        )),
    }
}

pub(super) fn map_persistence_error(error: OsmPoiRepositoryError, attempts: u32) -> Error {
    let kind = match error {
        OsmPoiRepositoryError::Connection { message } => {
            RepositoryPersistenceErrorKind::Connection { message }
        }
        OsmPoiRepositoryError::Query { message } => {
            RepositoryPersistenceErrorKind::Query { message }
        }
    };

    map_repository_persistence_error(RepositoryErrorContext {
        context: "enrichment persistence",
        attempts,
        kind,
    })
}

pub(super) fn map_provenance_persistence_error(
    error: EnrichmentProvenanceRepositoryError,
    attempts: u32,
) -> Error {
    let unavailable_prefix =
        format!("enrichment provenance persistence unavailable after {attempts} attempts");
    let failed_prefix =
        format!("enrichment provenance persistence failed after {attempts} attempts");

    map_enrichment_provenance_repository_error(
        error,
        unavailable_prefix.as_str(),
        failed_prefix.as_str(),
    )
}
