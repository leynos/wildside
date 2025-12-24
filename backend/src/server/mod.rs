//! Server construction and middleware wiring.

mod config;
#[cfg(feature = "metrics")]
mod metrics;

pub use config::ServerConfig;

#[cfg(feature = "metrics")]
use metrics::MetricsLayer;

use actix_session::{
    config::{CookieContentSecurity, PersistentSession},
    storage::CookieSessionStore,
    SessionMiddleware,
};
use actix_web::cookie::{Key, SameSite};
use actix_web::dev::{Server, ServiceFactory, ServiceRequest, ServiceResponse};
use actix_web::{web, App, HttpServer};

#[cfg(debug_assertions)]
use backend::doc::ApiDoc;
use backend::domain::ports::{
    FixtureLoginService, FixtureRouteSubmissionService, FixtureUserInterestsCommand,
    FixtureUserProfileQuery, FixtureUsersQuery,
};
use backend::domain::UserOnboardingService;
use backend::inbound::http::health::{live, ready, HealthState};
use backend::inbound::http::routes::submit_route;
use backend::inbound::http::state::{HttpState, HttpStatePorts};
use backend::inbound::http::users::{current_user, list_users, login, update_interests};
use backend::inbound::ws;
use backend::inbound::ws::state::WsState;
use backend::Trace;
#[cfg(debug_assertions)]
use utoipa::OpenApi;
#[cfg(debug_assertions)]
use utoipa_swagger_ui::SwaggerUi;

use std::sync::Arc;

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
        .service(submit_route);

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
    let http_state = web::Data::new(HttpState::new(HttpStatePorts::new(
        Arc::new(FixtureLoginService),
        Arc::new(FixtureUsersQuery),
        Arc::new(FixtureUserProfileQuery),
        Arc::new(FixtureUserInterestsCommand),
        Arc::new(FixtureRouteSubmissionService),
    )));
    let ws_state = web::Data::new(WsState::new(Arc::new(UserOnboardingService)));
    let ServerConfig {
        key,
        cookie_secure,
        same_site,
        bind_addr,
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
