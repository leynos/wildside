//! Server construction and middleware wiring.

mod config;
#[cfg(feature = "metrics")]
mod metrics;

pub use config::ServerConfig;

#[cfg(feature = "metrics")]
use metrics::MetricsLayer;

use actix_session::{
    SessionMiddleware,
    config::{CookieContentSecurity, PersistentSession},
    storage::CookieSessionStore,
};
use actix_web::cookie::{Key, SameSite};
use actix_web::dev::{Server, ServiceFactory, ServiceRequest, ServiceResponse};
use actix_web::{App, HttpServer, web};

use backend::Trace;
#[cfg(debug_assertions)]
use backend::doc::ApiDoc;
#[cfg(feature = "metrics")]
use backend::domain::ports::NoOpIdempotencyMetrics;
use backend::domain::ports::{
    CatalogueRepository, DescriptorRepository, FixtureCatalogueRepository,
    FixtureDescriptorRepository, FixtureLoginService, FixtureRouteAnnotationsCommand,
    FixtureRouteAnnotationsQuery, FixtureRouteSubmissionService, FixtureUserInterestsCommand,
    FixtureUserPreferencesCommand, FixtureUserPreferencesQuery, FixtureUserProfileQuery,
    FixtureUsersQuery, RouteAnnotationsCommand, RouteAnnotationsQuery, RouteSubmissionService,
    UserPreferencesCommand, UserPreferencesQuery,
};
use backend::domain::{
    RouteAnnotationsService, RouteSubmissionServiceImpl, UserOnboardingService,
    UserPreferencesService,
};
use backend::inbound::http::annotations::{get_annotations, update_progress, upsert_note};
use backend::inbound::http::catalogue::{get_descriptors, get_explore_catalogue};
use backend::inbound::http::health::{HealthState, live, ready};
use backend::inbound::http::preferences::{get_preferences, update_preferences};
use backend::inbound::http::routes::submit_route;
use backend::inbound::http::state::{HttpState, HttpStatePorts};
use backend::inbound::http::users::{current_user, list_users, login, update_interests};
use backend::inbound::ws;
use backend::inbound::ws::state::WsState;
#[cfg(feature = "metrics")]
use backend::outbound::metrics::PrometheusIdempotencyMetrics;
use backend::outbound::persistence::DieselIdempotencyRepository;
use backend::outbound::persistence::{
    DbPool, DieselCatalogueRepository, DieselDescriptorRepository, DieselRouteAnnotationRepository,
    DieselUserPreferencesRepository,
};
#[cfg(debug_assertions)]
use utoipa::OpenApi;
#[cfg(debug_assertions)]
use utoipa_swagger_ui::SwaggerUi;

use std::sync::Arc;

/// Build the route submission service based on configuration.
///
/// Uses the real DB-backed implementation when a pool is available, otherwise
/// falls back to the fixture for tests. When the metrics feature is enabled,
/// Prometheus-backed idempotency metrics are registered if both a DB pool and
/// Prometheus registry are available; otherwise, a no-op metrics implementation
/// is used.
///
/// # Parameters
/// - `config`: server configuration containing optional DB pool and Prometheus registry.
///
/// # Returns
/// An `Arc<dyn RouteSubmissionService>` wrapping either:
/// - `RouteSubmissionServiceImpl` with DB-backed storage and metrics (real or no-op).
/// - `FixtureRouteSubmissionService` when no DB pool is configured.
///
/// # Errors
/// Returns [`std::io::Error`] if Prometheus metric registration fails.
#[cfg(feature = "metrics")]
fn build_route_submission_service(
    config: &ServerConfig,
) -> std::io::Result<Arc<dyn RouteSubmissionService>> {
    match (&config.db_pool, &config.prometheus) {
        (Some(pool), Some(prom)) => {
            let idempotency_metrics =
                PrometheusIdempotencyMetrics::new(&prom.registry).map_err(|e| {
                    std::io::Error::other(format!("idempotency metrics registration failed: {e}"))
                })?;
            Ok(Arc::new(RouteSubmissionServiceImpl::new(
                Arc::new(DieselIdempotencyRepository::new(pool.clone())),
                Arc::new(idempotency_metrics),
            )))
        }
        (Some(pool), None) => Ok(Arc::new(RouteSubmissionServiceImpl::new(
            Arc::new(DieselIdempotencyRepository::new(pool.clone())),
            Arc::new(NoOpIdempotencyMetrics),
        ))),
        (None, _) => Ok(Arc::new(FixtureRouteSubmissionService)),
    }
}

/// Build the route submission service based on configuration.
///
/// Uses the real DB-backed implementation when a pool is available, otherwise
/// falls back to the fixture for tests. When the metrics feature is disabled,
/// a no-op metrics implementation is always used.
///
/// # Parameters
/// - `config`: server configuration containing optional DB pool.
///
/// # Returns
/// An `Arc<dyn RouteSubmissionService>` wrapping either:
/// - `RouteSubmissionServiceImpl` with DB-backed storage and no-op metrics.
/// - `FixtureRouteSubmissionService` when no DB pool is configured.
#[cfg(not(feature = "metrics"))]
fn build_route_submission_service(
    config: &ServerConfig,
) -> std::io::Result<Arc<dyn RouteSubmissionService>> {
    match &config.db_pool {
        Some(pool) => Ok(Arc::new(RouteSubmissionServiceImpl::with_noop_metrics(
            Arc::new(DieselIdempotencyRepository::new(pool.clone())),
        ))),
        None => Ok(Arc::new(FixtureRouteSubmissionService)),
    }
}

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

/// Helper to construct a service that depends on both a domain repository
/// and an idempotency repository, avoiding duplication of Arc wrapping.
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

/// Type alias for a function pointer that takes `Arc<S>` and returns `(Arc<Cmd>, Arc<Query>)` for casting/constructing command and query services.
type ServiceCast<S, Cmd, Query> = fn(Arc<S>) -> (Arc<Cmd>, Arc<Query>);

/// Struct holding prepared `fixtures: (Arc<Cmd>, Arc<Query>)` and the cast function `cast: ServiceCast<S, Cmd, Query>` for command/query service pairs.
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

/// Construct and return `(CatalogueRepository, DescriptorRepository)` by selecting
/// `DieselCatalogueRepository`/`DieselDescriptorRepository` when `config.db_pool` is
/// present, otherwise selecting `FixtureCatalogueRepository`/`FixtureDescriptorRepository`.
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
fn build_http_state(
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

#[derive(Clone)]
struct AppDependencies {
    health_state: web::Data<HealthState>,
    http_state: web::Data<HttpState>,
    ws_state: web::Data<WsState>,
    key: Key,
    cookie_secure: bool,
    same_site: SameSite,
}

fn build_app(
    deps: AppDependencies,
) -> App<
    impl ServiceFactory<
        ServiceRequest,
        Config = (),
        Response = ServiceResponse,
        Error = actix_web::Error,
        InitError = (),
    >,
> {
    let AppDependencies {
        health_state,
        http_state,
        ws_state,
        key,
        cookie_secure,
        same_site,
    } = deps;

    let session = SessionMiddleware::builder(CookieSessionStore::default(), key)
        .cookie_name("session".into())
        .cookie_path("/".into())
        .cookie_secure(cookie_secure)
        .cookie_http_only(true)
        .cookie_content_security(CookieContentSecurity::Private)
        .cookie_same_site(same_site)
        .session_lifecycle(
            PersistentSession::default().session_ttl(actix_web::cookie::time::Duration::hours(2)),
        )
        .build();

    let api = web::scope("/api/v1")
        .wrap(session)
        .service(login)
        .service(list_users)
        .service(current_user)
        .service(update_interests)
        .service(get_preferences)
        .service(update_preferences)
        .service(get_annotations)
        .service(upsert_note)
        .service(update_progress)
        .service(submit_route)
        .service(get_explore_catalogue)
        .service(get_descriptors);

    let app = App::new()
        .app_data(health_state)
        .app_data(http_state)
        .app_data(ws_state)
        .wrap(Trace)
        .service(api)
        .service(ws::ws_entry)
        .service(ready)
        .service(live);

    #[cfg(debug_assertions)]
    let app = app.service(SwaggerUi::new("/docs").url("/api-docs/openapi.json", ApiDoc::openapi()));
    #[cfg(not(debug_assertions))]
    let app = app;

    app
}

/// Construct an Actix HTTP server using the provided health state and configuration.
///
/// # Parameters
/// - `health_state`: shared readiness state updated once the server is initialised.
/// - `config`: pre-built [`ServerConfig`] containing session, binding, and optional metrics settings.
///
/// # Returns
/// A spawned [`Server`] that must be awaited to drive the listener.
///
/// # Errors
/// Propagates [`std::io::Error`] when binding the socket or starting the server fails.
pub fn create_server(
    health_state: web::Data<HealthState>,
    config: ServerConfig,
) -> std::io::Result<Server> {
    let server_health_state = health_state.clone();
    let route_submission = build_route_submission_service(&config)?;
    let http_state = build_http_state(&config, route_submission);
    let ws_state = web::Data::new(WsState::new(Arc::new(UserOnboardingService)));
    let ServerConfig {
        key,
        cookie_secure,
        same_site,
        bind_addr,
        db_pool: _,
        #[cfg(feature = "metrics")]
        prometheus,
    } = config;

    #[cfg(feature = "metrics")]
    let metrics_layer = MetricsLayer::from_option(prometheus);

    let server = HttpServer::new(move || {
        let app = build_app(AppDependencies {
            health_state: server_health_state.clone(),
            http_state: http_state.clone(),
            ws_state: ws_state.clone(),
            key: key.clone(),
            cookie_secure,
            same_site,
        });

        #[cfg(feature = "metrics")]
        let app = app.wrap(metrics_layer.clone());

        app
    })
    .bind(bind_addr)?
    .run();

    health_state.mark_ready();
    Ok(server)
}
