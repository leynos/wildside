//! Shared mapping helpers for enrichment provenance repository errors.

use crate::domain::Error;
use crate::domain::ports::EnrichmentProvenanceRepositoryError;

/// Maps provenance repository failures into domain errors with caller-provided
/// context prefixes.
pub(crate) fn map_enrichment_provenance_repository_error(
    error: EnrichmentProvenanceRepositoryError,
    unavailable_prefix: &str,
    failed_prefix: &str,
) -> Error {
    match error {
        EnrichmentProvenanceRepositoryError::Connection { message } => {
            Error::service_unavailable(format!("{unavailable_prefix}: {message}"))
        }
        EnrichmentProvenanceRepositoryError::Query { message } => {
            Error::internal(format!("{failed_prefix}: {message}"))
        }
    }
}
