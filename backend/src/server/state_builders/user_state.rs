//! Assembly of the service ports that make up HTTP user state.
//!
//! Keep pair builders private to this module and expose the complete bundle
//! only to the parent state composer.

use std::sync::Arc;

use backend::domain::ports::{
    FixtureLoginService, FixtureRouteAnnotationsCommand, FixtureRouteAnnotationsQuery,
    FixtureUserInterestsCommand, FixtureUserPreferencesCommand, FixtureUserPreferencesQuery,
    FixtureUserProfileQuery, FixtureUsersQuery, LoginService, PaginationErrorMetrics,
    RouteAnnotationsCommand, RouteAnnotationsQuery, UserInterestsCommand, UserPreferencesCommand,
    UserPreferencesQuery, UserProfileQuery, UsersQuery,
};
use backend::domain::{RouteAnnotationsService, UserPreferencesService};
use backend::outbound::persistence::{
    DbPool, DieselIdempotencyRepository, DieselLoginService, DieselRouteAnnotationRepository,
    DieselUserInterestsCommand, DieselUserPreferencesRepository, DieselUserProfileQuery,
    DieselUserRepository, DieselUsersQuery,
};

use super::ServerConfig;

/// Build a command/query service pair using real services when a pool is
/// available, otherwise using fixture implementations.
fn build_service_pair<Pool, Cmd, Query>(
    pool: &Option<Pool>,
    make_pair: impl FnOnce(&Pool) -> (Arc<Cmd>, Arc<Query>),
    fixtures: (Arc<Cmd>, Arc<Query>),
) -> (Arc<Cmd>, Arc<Query>)
where
    Cmd: ?Sized + 'static,
    Query: ?Sized + 'static,
{
    match pool {
        Some(pool) => make_pair(pool),
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

/// User-state ports selected together so DB-present mode cannot drift one port
/// at a time as HTTP state wiring evolves.
pub(super) struct UserStatePortsBundle {
    pub(super) login: Arc<dyn LoginService>,
    pub(super) users: Arc<dyn UsersQuery>,
    pub(super) profile: Arc<dyn UserProfileQuery>,
    pub(super) interests: Arc<dyn UserInterestsCommand>,
    pub(super) preferences: Arc<dyn UserPreferencesCommand>,
    pub(super) preferences_query: Arc<dyn UserPreferencesQuery>,
    pub(super) route_annotations: Arc<dyn RouteAnnotationsCommand>,
    pub(super) route_annotations_query: Arc<dyn RouteAnnotationsQuery>,
}

/// Build the login and users-query pair, injecting the pagination-error
/// recorder into both Diesel adapters.
fn build_login_users_pair(
    config: &ServerConfig,
    pagination_error_metrics: &Arc<dyn PaginationErrorMetrics>,
) -> (Arc<dyn LoginService>, Arc<dyn UsersQuery>) {
    build_service_pair(
        &config.db_pool,
        |pool| {
            (
                Arc::new(DieselLoginService::new(
                    DieselUserRepository::new(pool.clone()),
                    pagination_error_metrics.clone(),
                )) as Arc<dyn LoginService>,
                Arc::new(DieselUsersQuery::new(
                    DieselUserRepository::new(pool.clone()),
                    pagination_error_metrics.clone(),
                )) as Arc<dyn UsersQuery>,
            )
        },
        (
            Arc::new(FixtureLoginService) as Arc<dyn LoginService>,
            Arc::new(FixtureUsersQuery) as Arc<dyn UsersQuery>,
        ),
    )
}

fn build_profile_interests_pair(
    config: &ServerConfig,
    pagination_error_metrics: &Arc<dyn PaginationErrorMetrics>,
) -> (Arc<dyn UserProfileQuery>, Arc<dyn UserInterestsCommand>) {
    build_service_pair(
        &config.db_pool,
        |pool| {
            (
                Arc::new(DieselUserProfileQuery::new(
                    Arc::new(DieselUserRepository::new(pool.clone())),
                    pagination_error_metrics.clone(),
                )) as Arc<dyn UserProfileQuery>,
                Arc::new(DieselUserInterestsCommand::new(Arc::new(
                    DieselUserPreferencesRepository::new(pool.clone()),
                ))) as Arc<dyn UserInterestsCommand>,
            )
        },
        (
            Arc::new(FixtureUserProfileQuery) as Arc<dyn UserProfileQuery>,
            Arc::new(FixtureUserInterestsCommand) as Arc<dyn UserInterestsCommand>,
        ),
    )
}

macro_rules! build_idempotent_pair {
    (
        $fn_name:ident,
        $cmd_trait:ty,
        $query_trait:ty,
        $repo_ctor:expr,
        $service_ctor:expr,
        $fixture_cmd:path,
        $fixture_query:path
    ) => {
        fn $fn_name(config: &ServerConfig) -> (Arc<$cmd_trait>, Arc<$query_trait>) {
            build_idempotent_service_pair(
                config,
                $repo_ctor,
                $service_ctor,
                ServicePairFactory {
                    fixtures: (
                        Arc::new($fixture_cmd) as Arc<$cmd_trait>,
                        Arc::new($fixture_query) as Arc<$query_trait>,
                    ),
                    cast: |service| {
                        (
                            service.clone() as Arc<$cmd_trait>,
                            service as Arc<$query_trait>,
                        )
                    },
                },
            )
        }
    };
}

/// Build a command/query pair for services backed by a domain repository and
/// an idempotency repository.
///
/// This helper delegates branch selection to `build_service_pair`, and uses
/// `build_idempotent_service` when `ServerConfig.db_pool` is `Some(pool)`.
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
    let ServicePairFactory { fixtures, cast } = pair_factory;
    build_service_pair(
        &config.db_pool,
        |pool| {
            let service = Arc::new(build_idempotent_service(pool, make_repo, make_service));
            cast(service)
        },
        fixtures,
    )
}

build_idempotent_pair!(
    build_user_preferences_pair,
    dyn UserPreferencesCommand,
    dyn UserPreferencesQuery,
    DieselUserPreferencesRepository::new,
    UserPreferencesService::new,
    FixtureUserPreferencesCommand,
    FixtureUserPreferencesQuery
);

build_idempotent_pair!(
    build_route_annotations_pair,
    dyn RouteAnnotationsCommand,
    dyn RouteAnnotationsQuery,
    DieselRouteAnnotationRepository::new,
    RouteAnnotationsService::new,
    FixtureRouteAnnotationsCommand,
    FixtureRouteAnnotationsQuery
);

/// Assemble every HTTP user-state port into one bundle so DB-present and
/// fixture-backed wiring stay consistent.
pub(super) fn compose_user_state_ports(
    config: &ServerConfig,
    pagination_error_metrics: &Arc<dyn PaginationErrorMetrics>,
) -> UserStatePortsBundle {
    let (login, users) = build_login_users_pair(config, pagination_error_metrics);
    let (profile, interests) = build_profile_interests_pair(config, pagination_error_metrics);
    let (preferences, preferences_query) = build_user_preferences_pair(config);
    let (route_annotations, route_annotations_query) = build_route_annotations_pair(config);

    UserStatePortsBundle {
        login,
        users,
        profile,
        interests,
        preferences,
        preferences_query,
        route_annotations,
        route_annotations_query,
    }
}
