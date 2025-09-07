use actix_web::{get, HttpResponse};

/// Readiness probe. Return 200 when dependencies are initialised and the server can handle traffic.
#[utoipa::path(
    get,
    path = "/health/ready",
    responses(
        (status = 200, description = "Server is ready to handle traffic")
    )
)]
#[get("/health/ready")]
pub async fn ready() -> HttpResponse {
    HttpResponse::Ok().finish()
}

/// Liveness probe. Return 200 when the process is alive.
#[utoipa::path(
    get,
    path = "/health/live",
    responses(
        (status = 200, description = "Server is alive")
    )
)]
#[get("/health/live")]
pub async fn live() -> HttpResponse {
    HttpResponse::Ok().finish()
}
