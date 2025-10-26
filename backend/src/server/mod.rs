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

use backend::api::health::{live, ready, HealthState};
use backend::api::users::{list_users, login};
#[cfg(debug_assertions)]
use backend::doc::ApiDoc;
use backend::ws;
use backend::Trace;
#[cfg(debug_assertions)]
use utoipa::OpenApi;
#[cfg(debug_assertions)]
use utoipa_swagger_ui::SwaggerUi;

fn build_app(
    health_state: web::Data<HealthState>,
    key: Key,
    cookie_secure: bool,
    same_site: SameSite,
) -> App<
    impl ServiceFactory<
        ServiceRequest,
        Config = (),
        Response = ServiceResponse,
        Error = actix_web::Error,
        InitError = (),
    >,
> {
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
        .service(list_users);

    let app = App::new()
        .app_data(health_state)
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
        let app = build_app(
            server_health_state.clone(),
            key.clone(),
            cookie_secure,
            same_site,
        );

        #[cfg(feature = "metrics")]
        let app = app.wrap(metrics_layer.clone());

        app
    })
    .bind(bind_addr)?
    .run();

    health_state.mark_ready();
    Ok(server)
}
