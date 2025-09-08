//! Health endpoints: liveness & readiness probes for orchestration and load balancers.
//! Document endpoints in OpenAPI via Utoipa.
use actix_web::{get, web, HttpResponse};
use std::sync::atomic::{AtomicBool, Ordering};

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
        self.ready.store(true, Ordering::Relaxed);
    }

    /// Return readiness state.
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Relaxed)
    }
}

/// Readiness probe. Return 200 when dependencies are initialised and the server can handle traffic; return 503 otherwise.
#[utoipa::path(
    get,
    path = "/health/ready",
    responses(
        (status = 200, description = "Server is ready to handle traffic"),
        (status = 503, description = "Server is not ready")
    )
)]
#[get("/health/ready")]
pub async fn ready(state: web::Data<HealthState>) -> HttpResponse {
    if state.is_ready() {
        HttpResponse::Ok().finish()
    } else {
        HttpResponse::ServiceUnavailable().finish()
    }
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
