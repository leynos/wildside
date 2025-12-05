//! Domain primitives and aggregates.
//!
//! Purpose: Define strongly typed domain entities used by the API and
//! persistence layers. Keep types immutable and document invariants and
//! serialisation contracts (serde) in each type's Rustdoc.
//!
//! Public surface:
//! - Error (alias to `error::Error`) — domain-level error payload; HTTP
//!   mapping lives in the inbound adapters.
//! - ErrorCode (alias to `error::ErrorCode`) — stable error identifier shared
//!   across adapters.
//! - User (alias to `user::User`) — domain user identity and display name.
//! - LoginCredentials — validated username/password inputs for authentication.

pub mod auth;
pub mod error;
pub mod ports;
pub mod user;
pub mod user_events;
pub mod user_onboarding;

pub use self::auth::{LoginCredentials, LoginValidationError};
pub use self::error::{Error, ErrorCode, ErrorValidationError};
pub use self::user::{DisplayName, User, UserId, UserValidationError};
pub use self::user_events::{
    DisplayNameRejectedEvent, DisplayNameRejectionReason, UserCreatedEvent, UserEvent,
};
pub use self::user_onboarding::{DisplayNameSubmission, UserOnboardingService};

/// HTTP header name used to propagate trace identifiers.
pub const TRACE_ID_HEADER: &str = "trace-id";
