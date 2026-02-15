//! Domain ports and supporting types for the hexagonal boundary.

mod macros;
pub(crate) use macros::define_port_error;

mod cache_key;
mod catalogue_ingestion_repository;
mod catalogue_repository;
mod descriptor_ingestion_repository;
mod descriptor_repository;
mod example_data_runs_repository;
mod example_data_seed_repository;
mod idempotency_metrics;
mod idempotency_repository;
mod login_service;
mod route_annotation_repository;
mod route_annotations_command;
mod route_annotations_query;
mod route_cache;
mod route_metrics;
mod route_queue;
mod route_repository;
mod route_submission;
mod schema_snapshot_repository;
mod user_interests_command;
mod user_onboarding;
mod user_preferences_command;
mod user_preferences_query;
mod user_preferences_repository;
mod user_profile_query;
mod user_repository;
mod users_query;

pub use cache_key::{RouteCacheKey, RouteCacheKeyValidationError};
#[cfg(test)]
pub use catalogue_ingestion_repository::MockCatalogueIngestionRepository;
pub use catalogue_ingestion_repository::{
    CatalogueIngestionRepository, CatalogueIngestionRepositoryError,
    FixtureCatalogueIngestionRepository,
};
#[cfg(test)]
pub use catalogue_repository::MockCatalogueRepository;
pub use catalogue_repository::{
    CatalogueRepository, CatalogueRepositoryError, ExploreCatalogueSnapshot,
    FixtureCatalogueRepository,
};
#[cfg(test)]
pub use descriptor_ingestion_repository::MockDescriptorIngestionRepository;
pub use descriptor_ingestion_repository::{
    DescriptorIngestionRepository, DescriptorIngestionRepositoryError,
    FixtureDescriptorIngestionRepository,
};
#[cfg(test)]
pub use descriptor_repository::MockDescriptorRepository;
pub use descriptor_repository::{
    DescriptorRepository, DescriptorRepositoryError, DescriptorSnapshot,
    FixtureDescriptorRepository,
};
pub use example_data_runs_repository::{
    ExampleDataRunsError, ExampleDataRunsRepository, FixtureExampleDataRunsRepository,
    SeedingResult, try_seed_to_i64,
};
#[cfg(test)]
pub use example_data_seed_repository::MockExampleDataSeedRepository;
pub use example_data_seed_repository::{
    ExampleDataSeedRepository, ExampleDataSeedRepositoryError, ExampleDataSeedRequest,
    ExampleDataSeedUser,
};
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
#[cfg(test)]
pub use route_annotations_query::MockRouteAnnotationsQuery;
pub use route_annotations_query::{FixtureRouteAnnotationsQuery, RouteAnnotationsQuery};
pub use route_cache::{RouteCache, RouteCacheError};
pub use route_metrics::{RouteMetrics, RouteMetricsError};
pub use route_queue::{JobDispatchError, RouteQueue};
pub use route_repository::{RoutePersistenceError, RouteRepository};
pub use route_submission::{
    FixtureRouteSubmissionService, RouteSubmissionRequest, RouteSubmissionResponse,
    RouteSubmissionService, RouteSubmissionStatus,
};
#[cfg(test)]
pub use schema_snapshot_repository::MockSchemaSnapshotRepository;
pub use schema_snapshot_repository::{
    FixtureSchemaSnapshotRepository, SchemaSnapshotRepository, SchemaSnapshotRepositoryError,
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
pub use user_preferences_query::MockUserPreferencesQuery;
pub use user_preferences_query::{FixtureUserPreferencesQuery, UserPreferencesQuery};
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
