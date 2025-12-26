//! HTTP adapter mapping for domain errors.
//!
//! Purpose: keep the domain error type HTTP-agnostic while allowing Actix
//! handlers to turn domain failures into consistent JSON responses and status
//! codes.

use actix_web::{HttpResponse, ResponseError, http::StatusCode};
use tracing::error;

use crate::domain::{Error, ErrorCode, TRACE_ID_HEADER};

/// Convenient result alias for HTTP handlers.
pub type ApiResult<T> = Result<T, Error>;

fn status_for(code: ErrorCode) -> StatusCode {
    match code {
        ErrorCode::InvalidRequest => StatusCode::BAD_REQUEST,
        ErrorCode::Unauthorized => StatusCode::UNAUTHORIZED,
        ErrorCode::Forbidden => StatusCode::FORBIDDEN,
        ErrorCode::NotFound => StatusCode::NOT_FOUND,
        ErrorCode::Conflict => StatusCode::CONFLICT,
        ErrorCode::ServiceUnavailable => StatusCode::SERVICE_UNAVAILABLE,
        ErrorCode::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn redact_if_internal(error: &Error) -> Error {
    if matches!(error.code(), ErrorCode::InternalError) {
        let mut redacted = Error::internal("Internal server error");
        if let Some(id) = error.trace_id() {
            redacted = redacted.with_trace_id(id.to_owned());
        }
        redacted
    } else {
        error.clone()
    }
}

impl ResponseError for Error {
    fn status_code(&self) -> StatusCode {
        status_for(self.code())
    }

    fn error_response(&self) -> HttpResponse {
        let mut builder = HttpResponse::build(self.status_code());
        if let Some(id) = self.trace_id() {
            builder.insert_header((TRACE_ID_HEADER, id.to_owned()));
        }

        builder.json(redact_if_internal(self))
    }
}

impl From<actix_web::Error> for Error {
    fn from(err: actix_web::Error) -> Self {
        // Do not leak implementation details to clients.
        error!(error = %err, "actix error promoted to domain error");
        Error::internal("Internal server error")
    }
}

#[cfg(test)]
mod tests;
