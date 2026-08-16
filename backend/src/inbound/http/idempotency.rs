//! Helpers for parsing idempotency headers in HTTP handlers.

use actix_web::http::header::HeaderMap;
use utoipa::IntoParams;

use crate::domain::{Error, IdempotencyKey, IdempotencyKeyValidationError};

/// HTTP header name for idempotency keys.
pub const IDEMPOTENCY_KEY_HEADER: &str = "Idempotency-Key";

/// Optional UUID idempotency header used by mutating HTTP endpoints.
#[derive(IntoParams)]
#[into_params(parameter_in = Header)]
pub struct IdempotencyKeyHeader {
    /// UUID for idempotent requests.
    #[param(
        rename = "Idempotency-Key",
        value_type = String,
        required = false,
        format = "uuid"
    )]
    _value: Option<String>,
}

/// Extract the idempotency key from request headers.
pub fn extract_idempotency_key(
    headers: &HeaderMap,
) -> Result<Option<IdempotencyKey>, IdempotencyKeyValidationError> {
    let Some(header_value) = headers.get(IDEMPOTENCY_KEY_HEADER) else {
        return Ok(None);
    };

    let key_str = header_value
        .to_str()
        .map_err(|_| IdempotencyKeyValidationError::InvalidKey)?;

    IdempotencyKey::new(key_str).map(Some)
}

/// Map idempotency key validation errors to domain errors.
pub fn map_idempotency_key_error(err: IdempotencyKeyValidationError) -> Error {
    match err {
        IdempotencyKeyValidationError::EmptyKey => {
            Error::invalid_request("idempotency-key header must not be empty")
        }
        IdempotencyKeyValidationError::InvalidKey => {
            Error::invalid_request("idempotency-key header must be a valid uuid")
        }
    }
}
