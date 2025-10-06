#![cfg_attr(not(any(test, doctest)), deny(clippy::unwrap_used))]
#![cfg_attr(not(any(test, doctest)), forbid(clippy::expect_used))]
//! Backend entry-point: wires REST endpoints, WebSocket entry, and OpenAPI docs.

#[cfg(feature = "metrics")]
use actix_service::{
    boxed::{self, BoxService},
    Service, Transform,
};
use actix_session::{
    config::{CookieContentSecurity, PersistentSession},
    storage::CookieSessionStore,
    SessionMiddleware,
};
#[cfg(feature = "metrics")]
use actix_web::body::BoxBody;
use actix_web::cookie::{Key, SameSite};
use actix_web::dev::{Server, ServiceFactory, ServiceRequest, ServiceResponse};
#[cfg(feature = "metrics")]
use actix_web::middleware::{Compat, Identity};
use actix_web::{web, App, HttpServer};
#[cfg(feature = "metrics")]
use actix_web_prom::PrometheusMetricsBuilder;
#[cfg(feature = "metrics")]
use futures_util::future::LocalBoxFuture;
use std::env;
#[cfg(feature = "metrics")]
use std::sync::Arc;
use tracing::warn;
use tracing_subscriber::{fmt, EnvFilter};
#[cfg(debug_assertions)]
use utoipa_swagger_ui::SwaggerUi;
use zeroize::Zeroize;

use backend::api::health::{live, ready, HealthState};
use backend::api::users::{list_users, login};
#[cfg(debug_assertions)]
use backend::doc::ApiDoc;
use backend::ws;
use backend::Trace;
#[cfg(debug_assertions)]
use utoipa::OpenApi;

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

#[cfg(feature = "metrics")]
fn make_metrics(
) -> Result<actix_web_prom::PrometheusMetrics, Box<dyn std::error::Error + Send + Sync>> {
    PrometheusMetricsBuilder::new("wildside")
        .endpoint("/metrics")
        .build()
}

#[cfg(feature = "metrics")]
fn initialize_metrics<F, E>(make: F) -> Option<actix_web_prom::PrometheusMetrics>
where
    F: FnOnce() -> Result<actix_web_prom::PrometheusMetrics, E>,
    E: std::fmt::Display,
{
    match make() {
        Ok(metrics) => Some(metrics),
        Err(error) => {
            warn!(
                error = %error,
                "failed to initialize Prometheus metrics; continuing without metrics"
            );
            None
        }
    }
}

fn load_session_key() -> std::io::Result<Key> {
    let key_path =
        env::var("SESSION_KEY_FILE").unwrap_or_else(|_| "/var/run/secrets/session_key".into());
    match std::fs::read(&key_path) {
        Ok(mut bytes) => {
            if !cfg!(debug_assertions) && bytes.len() < 64 {
                return Err(std::io::Error::other(format!(
                    "session key at {key_path} too short: need >=64 bytes, got {}",
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
    let prometheus = initialize_metrics(make_metrics);
    let health_state = web::Data::new(HealthState::new());
    let server = create_server(
        health_state.clone(),
        key,
        cookie_secure,
        same_site,
        bind_address(),
        #[cfg(feature = "metrics")]
        prometheus,
    )?;
    server.await
}

#[cfg(feature = "metrics")]
fn create_server(
    health_state: web::Data<HealthState>,
    key: Key,
    cookie_secure: bool,
    same_site: SameSite,
    bind_address: (String, u16),
    prometheus: Option<actix_web_prom::PrometheusMetrics>,
) -> std::io::Result<Server> {
    let server_health_state = health_state.clone();
    let server = HttpServer::new(move || {
        let app = build_app(
            server_health_state.clone(),
            key.clone(),
            cookie_secure,
            same_site,
        );

        let middleware = MetricsLayer::from_option(prometheus.clone());

        app.wrap(middleware)
    })
    .bind(bind_address)?
    .run();

    health_state.mark_ready();
    Ok(server)
}

#[cfg(not(feature = "metrics"))]
fn create_server(
    health_state: web::Data<HealthState>,
    key: Key,
    cookie_secure: bool,
    same_site: SameSite,
    bind_address: (String, u16),
) -> std::io::Result<Server> {
    let server_health_state = health_state.clone();
    let server = HttpServer::new(move || {
        build_app(
            server_health_state.clone(),
            key.clone(),
            cookie_secure,
            same_site,
        )
    })
    .bind(bind_address)?
    .run();

    health_state.mark_ready();
    Ok(server)
}

#[cfg(feature = "metrics")]
#[derive(Clone)]
enum MetricsLayer {
    Enabled(Arc<actix_web_prom::PrometheusMetrics>),
    Disabled,
}

#[cfg(feature = "metrics")]
impl MetricsLayer {
    fn from_option(metrics: Option<actix_web_prom::PrometheusMetrics>) -> Self {
        match metrics {
            Some(metrics) => Self::Enabled(Arc::new(metrics)),
            None => Self::Disabled,
        }
    }
}

#[cfg(feature = "metrics")]
impl<S, B> Transform<S, ServiceRequest> for MetricsLayer
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = actix_web::Error> + 'static,
    B: actix_web::body::MessageBody + 'static,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = actix_web::Error;
    type InitError = ();
    type Transform = BoxService<ServiceRequest, ServiceResponse<BoxBody>, actix_web::Error>;
    type Future = LocalBoxFuture<'static, Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        match self.clone() {
            MetricsLayer::Enabled(metrics) => {
                let fut = Compat::new((*metrics).clone()).new_transform(service);
                Box::pin(async move {
                    let svc = fut.await?;
                    Ok(boxed::service(svc))
                })
            }
            MetricsLayer::Disabled => {
                let fut = Compat::new(Identity::default()).new_transform(service);
                Box::pin(async move {
                    let svc = fut.await?;
                    Ok(boxed::service(svc))
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::cookie::SameSite;

    #[cfg(feature = "metrics")]
    #[test]
    fn initialize_metrics_returns_none_on_error() {
        let metrics = initialize_metrics(|| -> Result<_, &str> { Err("boom") });
        assert!(metrics.is_none(), "expected metrics to be absent on error");
    }

    #[cfg(feature = "metrics")]
    #[test]
    fn initialize_metrics_returns_metrics_on_success() {
        let metrics = initialize_metrics(|| {
            PrometheusMetricsBuilder::new("test")
                .endpoint("/metrics")
                .build()
        });

        assert!(
            metrics.is_some(),
            "expected metrics to be present on success"
        );
    }

    #[cfg(feature = "metrics")]
    #[actix_rt::test]
    async fn create_server_marks_ready_without_metrics() {
        let state = web::Data::new(HealthState::new());
        assert!(!state.is_ready(), "state should start unready");

        let server = create_server(
            state.clone(),
            Key::generate(),
            false,
            SameSite::Lax,
            ("127.0.0.1".into(), 0),
            None,
        )
        .expect("server should build without metrics");

        assert!(state.is_ready(), "server creation should mark readiness");
        drop(server);
    }

    #[cfg(feature = "metrics")]
    #[actix_rt::test]
    async fn create_server_marks_ready_with_metrics() {
        let state = web::Data::new(HealthState::new());
        assert!(!state.is_ready(), "state should start unready");

        let prometheus = PrometheusMetricsBuilder::new("test")
            .endpoint("/metrics")
            .build()
            .expect("metrics should build for tests");

        let server = create_server(
            state.clone(),
            Key::generate(),
            false,
            SameSite::Lax,
            ("127.0.0.1".into(), 0),
            Some(prometheus),
        )
        .expect("server should build with metrics");

        assert!(state.is_ready(), "server creation should mark readiness");
        drop(server);
    }

    #[cfg(not(feature = "metrics"))]
    #[actix_rt::test]
    async fn create_server_marks_ready_without_metrics() {
        let state = web::Data::new(HealthState::new());
        assert!(!state.is_ready(), "state should start unready");

        let server = create_server(
            state.clone(),
            Key::generate(),
            false,
            SameSite::Lax,
            ("127.0.0.1".into(), 0),
        )
        .expect("server should build without metrics");

        assert!(state.is_ready(), "server creation should mark readiness");
        drop(server);
    }
}
