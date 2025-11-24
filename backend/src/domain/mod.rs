//! Domain primitives and aggregates.
//!
//! Purpose: Define strongly typed domain entities used by the API and
//! persistence layers. Keep types immutable and document invariants and
//! serialisation contracts (serde) in each type's Rustdoc.
//!
//! Public surface:
//! - DomainError (alias to `error::DomainError`) — transport-agnostic error payload.
//! - ErrorCode (alias to `error::ErrorCode`) — stable error identifier.
//! - User (alias to `user::User`) — domain user identity and display name.
//! - LoginCredentials — validated username/password inputs for authentication.

pub mod auth;
pub mod error;
pub mod ports;
pub mod user;

pub use self::auth::{LoginCredentials, LoginValidationError};
pub use self::error::{DomainError, ErrorCode, ErrorValidationError};
pub use self::user::{DisplayName, User, UserId, UserValidationError};

/// Convenient domain result alias.
///
/// # Examples
/// ```
/// use backend::domain::{DomainError, DomainResult, ErrorCode};
///
/// fn sample_operation() -> DomainResult<()> {
///     Err(DomainError::not_found("missing"))
/// }
/// ```
pub type DomainResult<T> = Result<T, DomainError>;
