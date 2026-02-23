//! Internal helpers for offline bundle services.

use crate::domain::ports::{IdempotencyRepositoryError, OfflineBundleRepositoryError};
use crate::domain::{Error, IdempotencyKey, PayloadHash, UserId};

pub(crate) fn map_bundle_repository_error(error: OfflineBundleRepositoryError) -> Error {
    match error {
        OfflineBundleRepositoryError::Connection { message } => {
            Error::service_unavailable(format!("offline bundle repository unavailable: {message}"))
        }
        OfflineBundleRepositoryError::Query { message } => {
            Error::internal(format!("offline bundle repository error: {message}"))
        }
    }
}

pub(crate) fn map_idempotency_error(error: IdempotencyRepositoryError) -> Error {
    match error {
        IdempotencyRepositoryError::Connection { message } => {
            Error::service_unavailable(format!("idempotency repository unavailable: {message}"))
        }
        IdempotencyRepositoryError::Query { message } => {
            Error::internal(format!("idempotency repository error: {message}"))
        }
        IdempotencyRepositoryError::Serialization { message } => Error::internal(format!(
            "idempotency repository serialization failed: {message}"
        )),
        IdempotencyRepositoryError::DuplicateKey { message } => {
            Error::internal(format!("unexpected idempotency key conflict: {message}"))
        }
    }
}

/// Inputs required for idempotent mutation orchestration.
pub(crate) struct IdempotentMutationContext {
    pub(crate) idempotency_key: Option<IdempotencyKey>,
    pub(crate) user_id: UserId,
    pub(crate) payload_hash: PayloadHash,
}
