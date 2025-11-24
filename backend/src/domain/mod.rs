//! Domain primitives and aggregates.
//!
//! Purpose: Define strongly typed domain entities used by the API and
//! persistence layers. Keep types immutable and document invariants and
//! serialisation contracts (serde) in each type's Rustdoc.
//!
//! Public surface:
//! - DomainError (alias to `error::DomainError`) — transport-agnostic failure.
//! - ErrorCode (alias to `error::ErrorCode`) — stable error identifier.
//! - User (alias to `user::User`) — domain user identity and display name.
//! - LoginCredentials — validated username/password inputs for authentication.

pub mod auth;
pub mod error;
pub mod ports;
pub mod user;

pub use self::auth::{LoginCredentials, LoginValidationError};
pub use self::error::{DomainError, DomainErrorValidationError, ErrorCode};
pub use self::user::{DisplayName, User, UserId, UserValidationError};

/// HTTP header name used to propagate trace identifiers.
pub const TRACE_ID_HEADER: &str = "trace-id";

/// Convenient result alias for domain operations.
pub type DomainResult<T> = Result<T, DomainError>;
