mod api;
mod models;
mod ws;

use actix_web::{App, HttpServer};
use tracing_subscriber::{fmt, EnvFilter};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use api::users::__path_list_users;
use api::users::list_users;
use models::user::User;

#[derive(OpenApi)]
#[openapi(paths(list_users), components(schemas(User)), tags((name = "users")))]
struct ApiDoc;

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
