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
        Ok(bytes) => Key::from(&bytes),
        Err(e) => {
            warn!(path = %key_path, error = %e, "failed to read session key; using temporary key");
            Key::generate()
        }
    };

    HttpServer::new(move || {
        let session_middleware =
            SessionMiddleware::builder(CookieSessionStore::default(), key.clone())
                .cookie_secure(true)
                .cookie_http_only(true)
                .cookie_same_site(SameSite::Lax)
                .build();

        let api = web::scope("/api")
            .wrap(session_middleware)
            .service(login)
            .service(list_users);

        let app = App::new()
            .service(api)
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
