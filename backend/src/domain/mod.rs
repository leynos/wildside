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
//! - MutationType — discriminator for idempotency scopes (routes, notes, etc.).
//! - IdempotencyConfig — configurable TTL for idempotency records.
//! - UserPreferences — user preferences for interests, safety, and display.
//! - UnitSystem — metric or imperial unit display preference.
//! - RouteNote — user annotation on a route or POI.
//! - RouteNoteContent — content parameters for creating route notes.
//! - RouteProgress — progress tracking for a route walk.
//! - RouteAnnotations — aggregated notes and progress for a route.
//! - ExampleDataSeeder — startup seeding orchestration for example data.
//! - ExampleDataSeedOutcome — summary of seeding results and counts.
//! - SchemaDiagram — normalized table/relationship graph used for ER snapshots.
//! - SchemaTable — table descriptor used by `SchemaDiagram`.
//! - SchemaColumn — typed column descriptor used by `SchemaTable`.
//! - SchemaRelationship — foreign-key edge used by `SchemaDiagram`.
//! - render_mermaid_er_diagram — deterministic Mermaid ER rendering function.
//! - LocalizationMap and LocalizedStringSet — validated localization payloads.
//! - SemanticIconIdentifier — validated semantic icon key.
//! - Catalogue read-model entities (`RouteSummary`, `RouteCategory`, `Theme`,
//!   `RouteCollection`, `TrendingRouteHighlight`, `CommunityPick`).
//! - Descriptor entities (`Tag`, `Badge`, `SafetyToggle`, `SafetyPreset`,
//!   `InterestTheme`).
//! - Offline bundle entities (`OfflineBundle`, `BoundingBox`, `ZoomRange`) and
//!   related enums (`OfflineBundleKind`, `OfflineBundleStatus`).
//! - Walk entities (`WalkSession`, `WalkCompletionSummary`) and stat value
//!   objects (`WalkPrimaryStat`, `WalkSecondaryStat`).

pub mod annotations;
pub mod auth;
pub mod catalogue;
pub mod descriptors;
pub mod er_diagram;
pub mod error;
#[cfg(feature = "example-data")]
pub mod example_data;
pub mod idempotency;
pub mod interest_theme;
pub mod localization;
pub mod offline;
pub mod offline_bundle_service;
pub mod ports;
pub mod preferences;
pub mod preferences_service;
pub mod route_submission;
pub mod semantic_icon_identifier;
mod slug;
pub mod trace_id;
pub mod user;
pub mod user_events;
pub mod user_interests;
pub mod user_onboarding;
pub mod walk_session_service;
pub mod walks;

pub use self::annotations::service::RouteAnnotationsService;
pub use self::annotations::{
    RouteAnnotations, RouteNote, RouteNoteBuilder, RouteNoteContent, RouteProgress,
    RouteProgressBuilder,
};
pub use self::auth::{LoginCredentials, LoginValidationError};
pub use self::catalogue::{
    CatalogueValidationError, CommunityPick, CommunityPickDraft, ImageAsset, RouteCategory,
    RouteCategoryDraft, RouteCollection, RouteCollectionDraft, RouteSummary, RouteSummaryDraft,
    Theme, ThemeDraft, TrendingRouteHighlight, TrendingRouteHighlightDraft,
};
pub use self::descriptors::{
    Badge, BadgeDraft, DescriptorValidationError, InterestTheme, SafetyPreset, SafetyPresetDraft,
    SafetyToggle, SafetyToggleDraft, Tag, TagDraft,
};
pub use self::er_diagram::{
    SchemaColumn, SchemaDiagram, SchemaRelationship, SchemaTable, render_mermaid_er_diagram,
};
pub use self::error::{Error, ErrorCode, ErrorValidationError};
#[cfg(feature = "example-data")]
pub use self::example_data::{ExampleDataSeedOutcome, ExampleDataSeeder, ExampleDataSeedingError};
pub use self::idempotency::{
    IdempotencyConfig, IdempotencyKey, IdempotencyKeyValidationError, IdempotencyLookupQuery,
    IdempotencyLookupResult, IdempotencyRecord, MutationType, ParseMutationTypeError, PayloadHash,
    PayloadHashError, canonicalize_and_hash,
};
pub use self::interest_theme::{InterestThemeId, InterestThemeIdValidationError};
pub use self::localization::{
    LocaleCode, LocalizationMap, LocalizationValidationError, LocalizedStringSet,
};
pub use self::offline::normalize_device_id as normalize_offline_device_id;
pub use self::offline::{
    BoundingBox, OfflineBundle, OfflineBundleDraft, OfflineBundleKind, OfflineBundleStatus,
    OfflineValidationError, ParseOfflineBundleKindError, ParseOfflineBundleStatusError, ZoomRange,
};
pub use self::offline_bundle_service::{OfflineBundleCommandService, OfflineBundleQueryService};
pub use self::preferences::{
    ParseUnitSystemError, UnitSystem, UserPreferences, UserPreferencesBuilder,
};
pub use self::preferences_service::UserPreferencesService;
pub use self::route_submission::RouteSubmissionServiceImpl;
pub use self::semantic_icon_identifier::{
    SemanticIconIdentifier, SemanticIconIdentifierValidationError,
};
pub use self::trace_id::TraceId;
pub use self::user::{DisplayName, User, UserId, UserValidationError};
pub use self::user_events::{DisplayNameRejectedEvent, UserCreatedEvent, UserEvent};
pub use self::user_interests::UserInterests;
pub use self::user_onboarding::UserOnboardingService;
pub use self::walk_session_service::{WalkSessionCommandService, WalkSessionQueryService};
pub use self::walks::{
    ParseWalkPrimaryStatKindError, ParseWalkSecondaryStatKindError, WalkCompletionSummary,
    WalkPrimaryStat, WalkPrimaryStatDraft, WalkPrimaryStatKind, WalkSecondaryStat,
    WalkSecondaryStatDraft, WalkSecondaryStatKind, WalkSession, WalkSessionDraft,
    WalkValidationError,
};

/// HTTP header name used to propagate trace identifiers.
pub const TRACE_ID_HEADER: &str = "trace-id";
