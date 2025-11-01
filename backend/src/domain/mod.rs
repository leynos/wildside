//! Domain primitives and aggregates.
//!
//! Purpose: Define strongly typed domain entities used by the API and
//! persistence layers. Keep types immutable and document invariants and
//! serialisation contracts (serde) in each type's Rustdoc.
//!
//! Public surface:
//! - Error (alias to `error::Error`) — API error response payload.
//! - ErrorCode (alias to `error::ErrorCode`) — stable error identifier.
//! - User (alias to `user::User`) — domain user identity and display name.

pub mod error;
pub mod user;

pub use self::error::{Error, ErrorCode, ErrorValidationError};
pub use self::user::{DisplayName, User, UserId, UserValidationError};

/// HTTP header name used to propagate trace identifiers.
pub const TRACE_ID_HEADER: &str = "trace-id";

/// Convenient API result alias.
///
/// # Examples
/// ```
/// use actix_web::HttpResponse;
/// use backend::domain::{ApiResult, Error};
///
/// fn handler() -> ApiResult<HttpResponse> {
///     Err(Error::forbidden("nope"))
/// }
/// ```
pub type ApiResult<T> = Result<T, Error>;
