//! Builders for HTTP state ports and repository-backed service pairs.

use std::sync::Arc;

use actix_web::web;

use backend::domain::ports::{
    CatalogueRepository, DescriptorRepository, FixtureCatalogueRepository,
    FixtureDescriptorRepository, FixtureLoginService, FixtureRouteAnnotationsCommand,
    FixtureRouteAnnotationsQuery, FixtureUserInterestsCommand, FixtureUserPreferencesCommand,
    FixtureUserPreferencesQuery, FixtureUserProfileQuery, FixtureUsersQuery,
    RouteAnnotationsCommand, RouteAnnotationsQuery, RouteSubmissionService, UserPreferencesCommand,
    UserPreferencesQuery,
};
use backend::domain::{RouteAnnotationsService, UserPreferencesService};
use backend::inbound::http::state::{HttpState, HttpStatePorts};
use backend::outbound::persistence::DieselIdempotencyRepository;
use backend::outbound::persistence::{
    DbPool, DieselCatalogueRepository, DieselDescriptorRepository, DieselRouteAnnotationRepository,
    DieselUserPreferencesRepository,
};

use super::ServerConfig;

/// Build a command/query service pair using real services when a pool is
/// available, otherwise using fixture implementations.
fn build_service_pair<S, Cmd, Query, MakeService, Cast>(
    pool: &Option<DbPool>,
    make_service: MakeService,
    fixtures: (Arc<Cmd>, Arc<Query>),
    cast: Cast,
) -> (Arc<Cmd>, Arc<Query>)
where
    S: 'static,
    Cmd: ?Sized + 'static,
    Query: ?Sized + 'static,
    MakeService: FnOnce(&DbPool) -> S,
    Cast: FnOnce(Arc<S>) -> (Arc<Cmd>, Arc<Query>),
{
    match pool {
        Some(pool) => {
            let service = Arc::new(make_service(pool));
            cast(service)
        }
        None => fixtures,
    }
}

/// Helper to construct a service that depends on both a domain repository and
/// an idempotency repository, avoiding duplication of `Arc` wrapping.
fn build_idempotent_service<R, S>(
    pool: &DbPool,
    make_repo: impl FnOnce(DbPool) -> R,
    make_service: impl FnOnce(Arc<R>, Arc<DieselIdempotencyRepository>) -> S,
) -> S
where
    R: 'static,
{
    let repo = Arc::new(make_repo(pool.clone()));
    let idempotency_repo = Arc::new(DieselIdempotencyRepository::new(pool.clone()));
    make_service(repo, idempotency_repo)
}

/// Type alias for a function pointer that takes `Arc<S>` and returns a pair of
/// service trait/object `Arc`s (`Arc<Cmd>`, `Arc<Query>`) used to cast and
/// construct command and query services.
type ServiceCast<S, Cmd, Query> = fn(Arc<S>) -> (Arc<Cmd>, Arc<Query>);

/// Struct holding prepared fixtures (`fixtures: (Arc<Cmd>, Arc<Query>)`) and
/// the cast function (`cast: ServiceCast<S, Cmd, Query>`) to produce
/// command/query service pairs for tests or initialization.
struct ServicePairFactory<S, Cmd: ?Sized, Query: ?Sized> {
    fixtures: (Arc<Cmd>, Arc<Query>),
    cast: ServiceCast<S, Cmd, Query>,
}

/// Build a command/query pair for services backed by a domain repository and
/// an idempotency repository.
fn build_idempotent_service_pair<R, S, Cmd, Query>(
    config: &ServerConfig,
    make_repo: impl FnOnce(DbPool) -> R,
    make_service: impl FnOnce(Arc<R>, Arc<DieselIdempotencyRepository>) -> S,
    pair_factory: ServicePairFactory<S, Cmd, Query>,
) -> (Arc<Cmd>, Arc<Query>)
where
    R: 'static,
    S: 'static,
    Cmd: ?Sized + 'static,
    Query: ?Sized + 'static,
{
    build_service_pair(
        &config.db_pool,
        |pool| build_idempotent_service(pool, make_repo, make_service),
        pair_factory.fixtures,
        pair_factory.cast,
    )
}

/// Construct and return `(CatalogueRepository, DescriptorRepository)` by
/// selecting `DieselCatalogueRepository`/`DieselDescriptorRepository` when
/// `config.db_pool` is present, otherwise selecting
/// `FixtureCatalogueRepository`/`FixtureDescriptorRepository`.
fn build_catalogue_services(
    config: &ServerConfig,
) -> (Arc<dyn CatalogueRepository>, Arc<dyn DescriptorRepository>) {
    match &config.db_pool {
        Some(pool) => (
            Arc::new(DieselCatalogueRepository::new(pool.clone())),
            Arc::new(DieselDescriptorRepository::new(pool.clone())),
        ),
        None => (
            Arc::new(FixtureCatalogueRepository),
            Arc::new(FixtureDescriptorRepository),
        ),
    }
}

/// Build the shared HTTP state from configured ports and fixture fallbacks.
pub(super) fn build_http_state(
    config: &ServerConfig,
    route_submission: Arc<dyn RouteSubmissionService>,
) -> web::Data<HttpState> {
    // TODO(#27): Wire remaining fixture ports (login, users, profile, interests)
    // to real DB-backed implementations once their adapters are ready.
    let (preferences, preferences_query) = build_idempotent_service_pair(
        config,
        DieselUserPreferencesRepository::new,
        UserPreferencesService::new,
        ServicePairFactory {
            fixtures: (
                Arc::new(FixtureUserPreferencesCommand) as Arc<dyn UserPreferencesCommand>,
                Arc::new(FixtureUserPreferencesQuery) as Arc<dyn UserPreferencesQuery>,
            ),
            cast: |service| {
                (
                    service.clone() as Arc<dyn UserPreferencesCommand>,
                    service as Arc<dyn UserPreferencesQuery>,
                )
            },
        },
    );
    let (route_annotations, route_annotations_query) = build_idempotent_service_pair(
        config,
        DieselRouteAnnotationRepository::new,
        RouteAnnotationsService::new,
        ServicePairFactory {
            fixtures: (
                Arc::new(FixtureRouteAnnotationsCommand) as Arc<dyn RouteAnnotationsCommand>,
                Arc::new(FixtureRouteAnnotationsQuery) as Arc<dyn RouteAnnotationsQuery>,
            ),
            cast: |service| {
                (
                    service.clone() as Arc<dyn RouteAnnotationsCommand>,
                    service as Arc<dyn RouteAnnotationsQuery>,
                )
            },
        },
    );
    let (catalogue, descriptors) = build_catalogue_services(config);

    web::Data::new(HttpState::new(HttpStatePorts {
        login: Arc::new(FixtureLoginService),
        users: Arc::new(FixtureUsersQuery),
        profile: Arc::new(FixtureUserProfileQuery),
        interests: Arc::new(FixtureUserInterestsCommand),
        preferences,
        preferences_query,
        route_annotations,
        route_annotations_query,
        route_submission,
        catalogue,
        descriptors,
    }))
}
