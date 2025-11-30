//! Health endpoints: liveness & readiness probes for orchestration and load balancers.
//! Document endpoints in OpenAPI via Utoipa.
use actix_web::{get, http::header, web, HttpResponse};
use std::sync::atomic::{AtomicBool, Ordering};

/// Shared health state for readiness and liveness checks.
/// Track readiness and whether the process should report itself as alive to orchestrators.
pub struct HealthState {
    ready: AtomicBool,
    live: AtomicBool,
}

impl Default for HealthState {
    fn default() -> Self {
        Self {
            ready: AtomicBool::new(false),
            live: AtomicBool::new(true),
        }
    }
}

impl HealthState {
    /// Create a new health state starting as not ready but live.
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark the service as ready.
    pub fn mark_ready(&self) {
        self.ready.store(true, Ordering::Release);
    }

    /// Flag the service as unhealthy so liveness checks fail fast during shutdown.
    pub fn mark_unhealthy(&self) {
        self.live.store(false, Ordering::Release);
    }

    /// Return readiness state.
    pub fn is_ready(&self) -> bool {
        self.ready.load(Ordering::Acquire)
    }

    /// Return liveness state. When false, liveness probes emit 503 to trigger restarts.
    pub fn is_alive(&self) -> bool {
        self.live.load(Ordering::Acquire)
    }

    fn probe_response(probe_ok: bool) -> HttpResponse {
        let mut response = if probe_ok {
            HttpResponse::Ok()
        } else {
            HttpResponse::ServiceUnavailable()
        };

        response
            .insert_header((header::CACHE_CONTROL, "no-store"))
            .finish()
    }
}

/// Readiness probe. Return 200 when dependencies are initialised and the server can handle traffic; return 503 otherwise.
#[utoipa::path(
    get,
    path = "/health/ready",
    tags = ["health"],
    security([]),
    responses(
        (status = 200, description = "Server is ready to handle traffic"),
        (
            status = 405,
            description = "Method not allowed; only GET probes are supported"
        ),
        (status = 503, description = "Server is not ready")
    )
)]
#[get("/health/ready")]
pub async fn ready(state: web::Data<HealthState>) -> HttpResponse {
    HealthState::probe_response(state.is_ready())
}

/// Liveness probe. Return 200 while the process is marked alive and 503 once draining.
/// Call `HealthState::mark_unhealthy` before graceful shutdown to surface the drain early.
#[utoipa::path(
    get,
    path = "/health/live",
    tags = ["health"],
    security([]),
    responses(
        (status = 200, description = "Server is alive"),
        (
            status = 405,
            description = "Method not allowed; only GET probes are supported"
        ),
        (
            status = 503,
            description = "Server is shutting down"
        )
    )
)]
#[get("/health/live")]
pub async fn live(state: web::Data<HealthState>) -> HttpResponse {
    HealthState::probe_response(state.is_alive())
}
