//! Backend entry-point: wires REST endpoints, WebSocket entry, and OpenAPI docs.

use actix_session::{storage::CookieSessionStore, SessionMiddleware};
use actix_web::cookie::{Key, SameSite};
use actix_web::{web, App, HttpServer};
#[cfg(feature = "metrics")]
use actix_web_prom::PrometheusMetricsBuilder;
use std::env;
use tracing::warn;
use tracing_subscriber::{fmt, EnvFilter};
#[cfg(debug_assertions)]
use utoipa::OpenApi;
#[cfg(debug_assertions)]
use utoipa_swagger_ui::SwaggerUi;

use backend::api::health::{live, ready, HealthState};
use backend::api::users::{list_users, login};
#[cfg(debug_assertions)]
use backend::doc::ApiDoc;
use backend::ws;
use backend::Trace;

/// Application bootstrap.
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    if let Err(e) = fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .json()
        .try_init()
    {
        warn!(error = %e, "tracing init failed");
    }

    let key_path =
        env::var("SESSION_KEY_FILE").unwrap_or_else(|_| "/var/run/secrets/session_key".into());
    let key = match std::fs::read(&key_path) {
        Ok(bytes) => Key::derive_from(&bytes),
        Err(e) => {
            let allow_dev = env::var("SESSION_ALLOW_EPHEMERAL").ok().as_deref() == Some("1");
            if cfg!(debug_assertions) || allow_dev {
                warn!(path = %key_path, error = %e, "using temporary session key (dev only)");
                Key::generate()
            } else {
                return Err(std::io::Error::other(format!(
                    "failed to read session key at {key_path}: {e}"
                )));
            }
        }
    };

    let cookie_secure = env::var("SESSION_COOKIE_SECURE")
        .map(|v| v != "0")
        .unwrap_or(true);

    let health_state = web::Data::new(HealthState::new());
    // Clone for server factory so readiness probe remains accessible.
    let server_health_state = health_state.clone();
    let server = HttpServer::new(move || {
        let app = build_app(server_health_state.clone(), key.clone(), cookie_secure);
        #[cfg(feature = "metrics")]
        let app = {
            let prometheus = make_metrics();
            app.wrap(prometheus)
        };
        app
    })
    .bind(("0.0.0.0", 8080))?;

    health_state.mark_ready();
    server.run().await
}

fn build_app(health_state: web::Data<HealthState>, key: Key, cookie_secure: bool) -> App<()> {
    let session = SessionMiddleware::builder(CookieSessionStore::default(), key)
        .cookie_name("session".into())
        .cookie_path("/".into())
        .cookie_secure(cookie_secure)
        .cookie_http_only(true)
        .cookie_same_site(SameSite::Lax)
        .build();

    let api = web::scope("/api/v1")
        .wrap(session)
        .service(login)
        .service(list_users);

    let mut app = App::new()
        .app_data(health_state)
        .wrap(Trace)
        .service(api)
        .service(ws::ws_entry)
        .service(ready)
        .service(live);

    #[cfg(debug_assertions)]
    {
        app = app.service(SwaggerUi::new("/docs").url("/api-docs/openapi.json", ApiDoc::openapi()));
    }

    app
}

#[cfg(feature = "metrics")]
fn make_metrics() -> actix_web_prom::PrometheusMetrics {
    PrometheusMetricsBuilder::new("wildside")
        .endpoint("/metrics")
        .build()
        .expect("configure Prometheus metrics")
}
