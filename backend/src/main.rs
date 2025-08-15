//! Backend entry-point: wires REST endpoints, WebSocket entry, and OpenAPI docs.

use actix_web::{get, App, HttpResponse, HttpServer};
use tracing::warn;
use tracing_subscriber::{fmt, EnvFilter};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use backend::api::users::list_users;
use backend::doc::ApiDoc;
use backend::ws;

#[get("/health/ready")]
async fn ready() -> HttpResponse {
    HttpResponse::Ok().finish()
}

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

    HttpServer::new(|| {
        let app = App::new()
            .service(list_users)
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
