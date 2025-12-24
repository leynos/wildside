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
//! - InterestThemeId — validated identifier for interest themes.
//! - UserInterests — selected interest themes for a user profile.
//! - LoginCredentials — validated username/password inputs for authentication.
//! - TraceId — per-request correlation identifier for tracing across systems.
//! - UserEvent (alias to `user_events::UserEvent`) — high-level user domain
//!   events, including `UserCreatedEvent` and `DisplayNameRejectedEvent`.
//! - UserOnboardingService — validated onboarding
//!   input and orchestration service for user creation workflows.
//! - IdempotencyKey — validated client-provided key for safe request retries.
//! - PayloadHash — SHA-256 hash of canonicalized request payload.
//! - IdempotencyRecord — stored record for idempotency tracking.
//! - IdempotencyLookupResult — outcome of idempotency key lookup.

pub mod auth;
pub mod error;
pub mod idempotency;
pub mod interest_theme;
pub mod ports;
pub mod route_submission;
pub mod trace_id;
pub mod user;
pub mod user_events;
pub mod user_interests;
pub mod user_onboarding;

pub use self::auth::{LoginCredentials, LoginValidationError};
pub use self::error::{Error, ErrorCode, ErrorValidationError};
pub use self::idempotency::{
    IdempotencyKey, IdempotencyKeyValidationError, IdempotencyLookupResult, IdempotencyRecord,
    PayloadHash, PayloadHashError, canonicalize_and_hash,
};
pub use self::idempotency::{
    canonicalize_and_hash, IdempotencyKey, IdempotencyKeyValidationError, IdempotencyLookupResult,
    IdempotencyRecord, PayloadHash, PayloadHashError,
};
pub use self::interest_theme::{InterestThemeId, InterestThemeIdValidationError};
pub use self::route_submission::RouteSubmissionServiceImpl;
pub use self::trace_id::TraceId;
pub use self::user::{DisplayName, User, UserId, UserValidationError};
pub use self::user_events::{DisplayNameRejectedEvent, UserCreatedEvent, UserEvent};
pub use self::user_interests::UserInterests;
pub use self::user_onboarding::UserOnboardingService;

/// HTTP header name used to propagate trace identifiers.
pub const TRACE_ID_HEADER: &str = "trace-id";
