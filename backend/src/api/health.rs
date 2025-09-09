//! Health endpoints: liveness & readiness probes for orchestration and load balancers.
//! Document endpoints in OpenAPI via Utoipa.
use actix_web::{get, http::header, web, HttpResponse};
use std::sync::atomic::{AtomicBool, Ordering};

/// Shared readiness state for health checks.
/// Gate /health/ready responses on this atomic flag.
#[derive(Default)]
pub struct HealthState {
    ready: AtomicBool,
}

impl HealthState {
    /// Create a new health state starting as not ready.
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark the service as ready.
    pub fn mark_ready(&self) {
        self.ready.store(true, Ordering::Release);
    }

    /// Return readiness state.
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }
}

/// Readiness probe. Return 200 when dependencies are initialised and the server can handle traffic; return 503 otherwise.
#[utoipa::path(
    get,
    path = "/health/ready",
    tags = ["health"],
    responses(
        (status = 200, description = "Server is ready to handle traffic"),
        (status = 503, description = "Server is not ready")
    )
)]
#[get("/health/ready")]
pub async fn ready(state: web::Data<HealthState>) -> HttpResponse {
    if state.is_ready() {
        HttpResponse::Ok()
            .insert_header((header::CACHE_CONTROL, "no-store"))
            .finish()
    } else {
        HttpResponse::ServiceUnavailable()
            .insert_header((header::CACHE_CONTROL, "no-store"))
            .finish()
    }
}

/// Liveness probe. Return 200 when the process is alive.
#[utoipa::path(
    get,
    path = "/health/live",
    tags = ["health"],
    responses(
        (status = 200, description = "Server is alive")
    )
)]
#[get("/health/live")]
pub async fn live() -> HttpResponse {
    HttpResponse::Ok()
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .finish()
}
