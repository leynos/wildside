//! Server construction and middleware wiring.

mod config;
#[cfg(feature = "metrics")]
mod metrics;
mod state_builders;

pub use config::ServerConfig;

#[cfg(feature = "metrics")]
use metrics::MetricsLayer;
use state_builders::build_http_state;

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
use backend::domain::ports::{FixtureRouteSubmissionService, RouteSubmissionService};
use backend::domain::{RouteSubmissionServiceImpl, UserOnboardingService};
use backend::inbound::http::annotations::{get_annotations, update_progress, upsert_note};
use backend::inbound::http::catalogue::{get_descriptors, get_explore_catalogue};
use backend::inbound::http::health::{HealthState, live, ready};
use backend::inbound::http::preferences::{get_preferences, update_preferences};
use backend::inbound::http::routes::submit_route;
use backend::inbound::http::state::HttpState;
use backend::inbound::http::users::{current_user, list_users, login, update_interests};
use backend::inbound::ws;
use backend::inbound::ws::state::WsState;
#[cfg(feature = "metrics")]
use backend::outbound::metrics::PrometheusIdempotencyMetrics;
use backend::outbound::persistence::DieselIdempotencyRepository;
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
