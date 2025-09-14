//! Backend entry-point: wires REST endpoints, WebSocket entry, and OpenAPI docs.

use actix_session::{config::PersistentSession, storage::CookieSessionStore, SessionMiddleware};
use actix_web::cookie::{Key, SameSite};
use actix_web::dev::{ServiceFactory, ServiceRequest, ServiceResponse};
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
use zeroize::Zeroize;

use backend::api::health::{live, ready, HealthState};
use backend::api::users::{list_users, login};
#[cfg(debug_assertions)]
use backend::doc::ApiDoc;
use backend::ws;
use backend::Trace;

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
        .cookie_same_site(same_site)
        .session_lifecycle(
            PersistentSession::default().session_ttl(actix_web::cookie::time::Duration::hours(2)),
        )
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
fn make_metrics(
) -> Result<actix_web_prom::PrometheusMetrics, Box<dyn std::error::Error + Send + Sync>> {
    PrometheusMetricsBuilder::new("wildside")
        .endpoint("/metrics")
        .build()
}

fn load_session_key() -> std::io::Result<Key> {
    let key_path =
        env::var("SESSION_KEY_FILE").unwrap_or_else(|_| "/var/run/secrets/session_key".into());
    match std::fs::read(&key_path) {
        Ok(mut bytes) => {
            if !cfg!(debug_assertions) && bytes.len() < 32 {
                return Err(std::io::Error::other(format!(
                    "session key at {key_path} too short: need >=32 bytes, got {}",
                    bytes.len()
                )));
            }
            let key = Key::derive_from(&bytes);
            bytes.zeroize();
            Ok(key)
        }
        Err(e) => {
            let allow_dev = env::var("SESSION_ALLOW_EPHEMERAL").ok().as_deref() == Some("1");
            if cfg!(debug_assertions) || allow_dev {
                warn!(path = %key_path, error = %e, "using temporary session key (dev only)");
                Ok(Key::generate())
            } else {
                Err(std::io::Error::other(format!(
                    "failed to read session key at {key_path}: {e}"
                )))
            }
        }
    }
}

fn cookie_secure_from_env() -> bool {
    match env::var("SESSION_COOKIE_SECURE") {
        Ok(v) => match v.to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "y" => true,
            "0" | "false" | "no" | "n" => false,
            other => {
                warn!(value = %other, "invalid SESSION_COOKIE_SECURE; defaulting to secure");
                true
            }
        },
        Err(_) => true,
    }
}

/// Determine the session SameSite policy, allowing an environment override.
///
/// Defaults to `Lax` in debug builds and `Strict` otherwise. `SESSION_SAMESITE`
/// can set `Strict`, `Lax`, or `None`; choosing `None` requires a secure cookie
/// and some browsers may block such third-party cookies entirely.
fn same_site_from_env(cookie_secure: bool) -> std::io::Result<SameSite> {
    let default_same_site = if cfg!(debug_assertions) {
        SameSite::Lax
    } else {
        SameSite::Strict
    };
    Ok(match env::var("SESSION_SAMESITE") {
        Ok(v) => match v.to_ascii_lowercase().as_str() {
            "lax" => SameSite::Lax,
            "strict" => SameSite::Strict,
            "none" => {
                if !cookie_secure && !cfg!(debug_assertions) {
                    return Err(std::io::Error::other(
                        "SESSION_SAMESITE=None requires SESSION_COOKIE_SECURE=1",
                    ));
                }
                SameSite::None
            }
            other => {
                if cfg!(debug_assertions) {
                    warn!(value = %other, "invalid SESSION_SAMESITE, using default");
                    default_same_site
                } else {
                    return Err(std::io::Error::other(format!(
                        "invalid SESSION_SAMESITE: {other}"
                    )));
                }
            }
        },
        Err(_) => default_same_site,
    })
}

fn bind_address() -> (String, u16) {
    (
        env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into()),
        match env::var("PORT") {
            Ok(p) => match p.parse::<u16>() {
                Ok(n) => n,
                Err(_) => {
                    warn!(value = %p, "invalid PORT; falling back to 8080");
                    8080u16
                }
            },
            Err(_) => 8080u16,
        },
    )
}

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

    let key = load_session_key()?;
    let cookie_secure = cookie_secure_from_env();
    let same_site = same_site_from_env(cookie_secure)?;
    #[cfg(feature = "metrics")]
    let prometheus = match make_metrics() {
        Ok(m) => m,
        Err(e) => {
            warn!(error = %e, "failed to initialise Prometheus metrics");
            return Err(std::io::Error::other(e));
        }
    };
    let health_state = web::Data::new(HealthState::new());
    let server_health_state = health_state.clone();
    let server = HttpServer::new(move || {
        let app = build_app(
            server_health_state.clone(),
            key.clone(),
            cookie_secure,
            same_site,
        );
        #[cfg(feature = "metrics")]
        let app = app.wrap(prometheus.clone());
        app
    })
    .bind(bind_address())?;

    let server = server.run();
    health_state.mark_ready();
    server.await
}
