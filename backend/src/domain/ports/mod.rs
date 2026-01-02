//! Domain ports and supporting types for the hexagonal boundary.

mod macros;
pub(crate) use macros::define_port_error;

mod cache_key;
mod idempotency_metrics;
mod idempotency_repository;
mod login_service;
mod route_annotation_repository;
mod route_annotations_command;
mod route_cache;
mod route_metrics;
mod route_queue;
mod route_repository;
mod route_submission;
mod user_interests_command;
mod user_onboarding;
mod user_preferences_command;
mod user_preferences_repository;
mod user_profile_query;
mod user_repository;
mod users_query;

pub use cache_key::{RouteCacheKey, RouteCacheKeyValidationError};
pub use idempotency_metrics::{
    IdempotencyMetricLabels, IdempotencyMetrics, IdempotencyMetricsError, NoOpIdempotencyMetrics,
};
#[cfg(test)]
pub use idempotency_repository::MockIdempotencyRepository;
pub use idempotency_repository::{
    FixtureIdempotencyRepository, IdempotencyRepository, IdempotencyRepositoryError,
};
pub use login_service::{FixtureLoginService, LoginService};
#[cfg(test)]
pub use route_annotation_repository::MockRouteAnnotationRepository;
pub use route_annotation_repository::{
    FixtureRouteAnnotationRepository, RouteAnnotationRepository, RouteAnnotationRepositoryError,
};
#[cfg(test)]
pub use route_annotations_command::MockRouteAnnotationsCommand;
pub use route_annotations_command::{
    DeleteNoteRequest, DeleteNoteResponse, FixtureRouteAnnotationsCommand, RouteAnnotationsCommand,
    UpdateProgressRequest, UpdateProgressResponse, UpsertNoteRequest, UpsertNoteResponse,
};
pub use route_cache::{RouteCache, RouteCacheError};
pub use route_metrics::{RouteMetrics, RouteMetricsError};
pub use route_queue::{JobDispatchError, RouteQueue};
pub use route_repository::{RoutePersistenceError, RouteRepository};
pub use route_submission::{
    FixtureRouteSubmissionService, RouteSubmissionRequest, RouteSubmissionResponse,
    RouteSubmissionService, RouteSubmissionStatus,
};
pub use user_interests_command::{FixtureUserInterestsCommand, UserInterestsCommand};
pub use user_onboarding::UserOnboarding;
#[cfg(test)]
pub use user_preferences_command::MockUserPreferencesCommand;
pub use user_preferences_command::{
    FixtureUserPreferencesCommand, UpdatePreferencesRequest, UpdatePreferencesResponse,
    UserPreferencesCommand,
};
#[cfg(test)]
pub use user_preferences_repository::MockUserPreferencesRepository;
pub use user_preferences_repository::{
    FixtureUserPreferencesRepository, UserPreferencesRepository, UserPreferencesRepositoryError,
};
pub use user_profile_query::{FixtureUserProfileQuery, UserProfileQuery};
pub use user_repository::{UserPersistenceError, UserRepository};
pub use users_query::{FixtureUsersQuery, UsersQuery};

#[cfg(test)]
mod tests;
