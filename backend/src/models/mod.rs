//! Domain data models.
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
pub use self::error::{Error, ErrorCode};
pub use self::user::User;

/// Convenient API result alias.
pub type ApiResult<T> = Result<T, Error>;
