//! Backend entry-point: wires REST endpoints, WebSocket entry, and OpenAPI docs.

use actix_web::{App, HttpServer};
use tracing_subscriber::{fmt, EnvFilter};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use backend::api::users::list_users;
use backend::doc::ApiDoc;
use backend::ws;

/// Application bootstrap.
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let _ = fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .json()
        .try_init();

    HttpServer::new(|| {
        App::new()
            .service(list_users)
            .service(ws::ws_entry)
            .service(SwaggerUi::new("/docs").url("/api-docs/openapi.json", ApiDoc::openapi()))
    })
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}
