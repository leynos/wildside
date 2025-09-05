//! Backend entry-point: wires REST endpoints, WebSocket entry, and OpenAPI docs.

use actix_session::{storage::CookieSessionStore, SessionMiddleware};
use actix_web::cookie::{Key, SameSite};
use actix_web::{get, web, App, HttpResponse, HttpServer};
use std::env;
use tracing::warn;
use tracing_subscriber::{fmt, EnvFilter};
#[cfg(debug_assertions)]
use utoipa::OpenApi;
#[cfg(debug_assertions)]
use utoipa_swagger_ui::SwaggerUi;

use backend::api::users::{list_users, login};
#[cfg(debug_assertions)]
use backend::doc::ApiDoc;
use backend::middleware::Trace;
use backend::ws;

/// Readiness probe. Return 200 when dependencies are initialised and the server can handle traffic.
#[get("/health/ready")]
async fn ready() -> HttpResponse {
    HttpResponse::Ok().finish()
}

/// Liveness probe. Return 200 when the process is alive.
#[get("/health/live")]
async fn live() -> HttpResponse {
    HttpResponse::Ok().finish()
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

    HttpServer::new(move || {
        let session_middleware =
            SessionMiddleware::builder(CookieSessionStore::default(), key.clone())
                .cookie_name("session".to_owned())
                .cookie_path("/".to_owned())
                .cookie_secure(cookie_secure)
                .cookie_http_only(true)
                .cookie_same_site(SameSite::Lax)
                .build();

        let api = web::scope("/api/v1")
            .wrap(session_middleware)
            .service(login)
            .service(list_users);

        let app = App::new()
<<<<<<< HEAD
            .service(api)
||||||| parent of 2ea98fb (Add request trace IDs and error helpers)
            .service(list_users)
=======
            .wrap(Trace)
            .service(list_users)
>>>>>>> 2ea98fb (Add request trace IDs and error helpers)
            .service(ws::ws_entry)
            .service(ready)
            .service(live);

        #[cfg(debug_assertions)]
        let app =
            app.service(SwaggerUi::new("/docs").url("/api-docs/openapi.json", ApiDoc::openapi()));

        app
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}
