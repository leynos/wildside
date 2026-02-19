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
///
/// # Examples
///
/// ```ignore
/// use std::sync::Arc;
///
/// // Dummy builder for examples; in real code this comes from ServerConfig.
/// fn example_pool() -> DbPool { todo!("provide a test DbPool") }
///
/// #[derive(Debug)]
/// struct DemoService(&'static str);
///
/// // `MakeService` closure: convert a `&DbPool` into a concrete service.
/// let make_service = |_pool: &DbPool| DemoService("db-backed");
///
/// // Fixtures used when `pool: &Option<DbPool>` is `None`.
/// let fixtures = (
///     Arc::new(String::from("fixture-cmd")),
///     Arc::new(String::from("fixture-query")),
/// );
///
/// // `cast` closure maps `Arc<DemoService>` into `(Arc<Cmd>, Arc<Query>)`.
/// let cast = |service: Arc<DemoService>| {
///     (
///         Arc::new(format!("{}-cmd", service.0)),
///         Arc::new(format!("{}-query", service.0)),
///     )
/// };
///
/// // Branch 1: `Some(pool)` uses `make_service` and `cast`.
/// let pool = example_pool();
/// let some_pool: Option<DbPool> = Some(pool);
/// let from_db = build_service_pair(&some_pool, make_service, fixtures.clone(), cast);
/// assert_eq!(from_db.0.as_str(), "db-backed-cmd");
/// assert_eq!(from_db.1.as_str(), "db-backed-query");
///
/// // Branch 2: `None` returns the fixture tuple untouched.
/// let none_pool: Option<DbPool> = None;
/// let from_fixtures = build_service_pair(
///     &none_pool,
///     |_pool: &DbPool| DemoService("unused"),
///     fixtures.clone(),
///     |service: Arc<DemoService>| {
///         (
///             Arc::new(format!("{}-cmd", service.0)),
///             Arc::new(format!("{}-query", service.0)),
///         )
///     },
/// );
/// assert_eq!(from_fixtures.0.as_str(), "fixture-cmd");
/// assert_eq!(from_fixtures.1.as_str(), "fixture-query");
/// ```
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
///
/// # Examples
///
/// ```ignore
/// use std::sync::Arc;
/// use backend::outbound::persistence::{DbPool, DieselIdempotencyRepository};
/// #[derive(Debug)]
/// struct DemoRepo;
/// #[derive(Debug, PartialEq, Eq)]
/// struct DemoService { label: &'static str }
/// fn example_pool() -> DbPool { todo!("supply a DbPool for tests") }
///
/// let pool = example_pool();
/// let make_repo = |_pool: DbPool| DemoRepo;
/// let make_service = |_repo: Arc<DemoRepo>, _idempotency: Arc<DieselIdempotencyRepository>| {
///     DemoService { label: "constructed" }
/// };
/// let service = build_idempotent_service(&pool, make_repo, make_service);
/// assert_eq!(service.label, "constructed");
/// ```
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
///
/// This helper delegates branch selection to `build_service_pair`, and uses
/// `build_idempotent_service` when `ServerConfig.db_pool` is `Some(pool)`.
///
/// # Examples
///
/// ```ignore
/// use std::sync::Arc;
/// use actix_web::cookie::{Key, SameSite};
/// use backend::outbound::persistence::{DbPool, DieselIdempotencyRepository};
/// #[derive(Debug)]
/// struct DemoRepo;
/// #[derive(Debug)]
/// struct DemoService(&'static str);
/// fn cast_service(service: Arc<DemoService>) -> (Arc<String>, Arc<String>) {
///     (
///         Arc::new(format!("{}-cmd", service.0)),
///         Arc::new(format!("{}-query", service.0)),
///     )
/// }
/// fn base_config() -> ServerConfig {
///     ServerConfig::new(
///         Key::generate(),
///         false,
///         SameSite::Lax,
///         "127.0.0.1:0".parse().expect("valid socket address"),
///     )
/// }
/// fn example_pool() -> DbPool { todo!("supply a DbPool for tests") }
///
/// let pair_with_db: (Arc<String>, Arc<String>) = build_idempotent_service_pair(
///     &base_config().with_db_pool(example_pool()),
///     |_pool: DbPool| DemoRepo,
///     |_repo: Arc<DemoRepo>, _idem: Arc<DieselIdempotencyRepository>| DemoService("db"),
///     ServicePairFactory {
///         fixtures: (Arc::new("fixture-cmd".to_string()), Arc::new("fixture-query".to_string())),
///         cast: cast_service,
///     },
/// );
/// assert_eq!(pair_with_db.0.as_str(), "db-cmd");
/// assert_eq!(pair_with_db.1.as_str(), "db-query");
/// let pair_without_db: (Arc<String>, Arc<String>) = build_idempotent_service_pair(
///     &base_config(),
///     |_pool: DbPool| DemoRepo,
///     |_repo: Arc<DemoRepo>, _idem: Arc<DieselIdempotencyRepository>| DemoService("unused"),
///     ServicePairFactory {
///         fixtures: (Arc::new("fixture-cmd".to_string()), Arc::new("fixture-query".to_string())),
///         cast: cast_service,
///     },
/// );
/// assert_eq!(pair_without_db.0.as_str(), "fixture-cmd");
/// assert_eq!(pair_without_db.1.as_str(), "fixture-query");
/// ```
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
///
/// # Examples
///
/// ```ignore
/// use std::sync::Arc;
/// use actix_web::cookie::{Key, SameSite};
/// use backend::domain::ports::{CatalogueRepository, DescriptorRepository};
/// use backend::outbound::persistence::DbPool;
/// fn base_config() -> ServerConfig {
///     ServerConfig::new(
///         Key::generate(),
///         false,
///         SameSite::Lax,
///         "127.0.0.1:0".parse().expect("valid socket address"),
///     )
/// }
/// fn example_pool() -> DbPool { todo!("supply a DbPool for tests") }
///
/// let config_with_db = base_config().with_db_pool(example_pool());
/// let (catalogue_db, descriptors_db): (
///     Arc<dyn CatalogueRepository>,
///     Arc<dyn DescriptorRepository>,
/// ) = build_catalogue_services(&config_with_db);
/// // `Some(db_pool)` branch uses DieselCatalogueRepository and
/// // DieselDescriptorRepository, then upcasts to trait objects.
/// let _ = (catalogue_db, descriptors_db);
/// let config_without_db = base_config();
/// let (catalogue_fixture, descriptors_fixture): (
///     Arc<dyn CatalogueRepository>,
///     Arc<dyn DescriptorRepository>,
/// ) = build_catalogue_services(&config_without_db);
/// // `None` branch uses FixtureCatalogueRepository and
/// // FixtureDescriptorRepository.
/// let _ = (catalogue_fixture, descriptors_fixture);
/// ```
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
///
/// # Examples
///
/// ```ignore
/// use std::sync::Arc;
/// use actix_web::cookie::{Key, SameSite};
/// use backend::domain::ports::{FixtureRouteSubmissionService, RouteSubmissionService};
/// use backend::outbound::persistence::DbPool;
/// fn base_config() -> ServerConfig {
///     ServerConfig::new(
///         Key::generate(),
///         false,
///         SameSite::Lax,
///         "127.0.0.1:0".parse().expect("valid socket address"),
///     )
/// }
/// fn example_pool() -> DbPool { todo!("supply a DbPool for tests") }
///
/// let config = base_config().with_db_pool(example_pool());
/// let route_submission =
///     Arc::new(FixtureRouteSubmissionService) as Arc<dyn RouteSubmissionService>;
/// let http_state: actix_web::web::Data<HttpState> = build_http_state(&config, route_submission);
///
/// let state = http_state.get_ref();
/// // All ports are populated: login, users, profile, interests, preferences,
/// // preferences_query, route_annotations, route_annotations_query,
/// // route_submission, catalogue, descriptors.
/// let _login = state.login.clone();
/// let _preferences = state.preferences.clone();
/// let _ = (
///     state.users.clone(),
///     state.profile.clone(),
///     state.interests.clone(),
///     state.preferences_query.clone(),
///     state.route_annotations.clone(),
///     state.route_annotations_query.clone(),
///     state.route_submission.clone(),
///     state.catalogue.clone(),
///     state.descriptors.clone(),
/// );
/// ```
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
