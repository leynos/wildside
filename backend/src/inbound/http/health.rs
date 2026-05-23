//! Health endpoints: liveness & readiness probes for orchestration and load balancers.
//! Document endpoints in OpenAPI via Utoipa.
use actix_web::{HttpResponse, get, http::header, web};
use serde::Serialize;
use std::collections::BTreeMap;

use crate::domain::ProcessHealth;
use crate::domain::ports::HealthObserver;

/// Backwards-compatible name for the domain-owned process health state.
pub type HealthState = ProcessHealth;

#[derive(Serialize)]
struct HealthProbeCheck {
    status: &'static str,
}

#[derive(Serialize)]
struct HealthProbeBody {
    status: &'static str,
    checks: BTreeMap<&'static str, HealthProbeCheck>,
}

impl HealthProbeBody {
    fn new(check_name: &'static str, probe_ok: bool) -> Self {
        let status = if probe_ok { "pass" } else { "fail" };
        let checks = BTreeMap::from([(check_name, HealthProbeCheck { status })]);
        Self { status, checks }
    }
}

fn probe_response(check_name: &'static str, probe_ok: bool) -> HttpResponse {
    let mut response = if probe_ok {
        HttpResponse::Ok()
    } else {
        HttpResponse::ServiceUnavailable()
    };

    response
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .json(HealthProbeBody::new(check_name, probe_ok))
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
    probe_response("readiness", state.observe_readiness().is_healthy())
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
    probe_response("liveness", state.observe_liveness().is_healthy())
}

#[cfg(test)]
mod tests {
    //! Tests for HTTP health probe response mapping.

    use super::{HealthState, live, ready};
    use actix_web::http::StatusCode;
    use actix_web::{App, http::header, test, web};
    use rstest::{fixture, rstest};

    #[fixture]
    fn health_state() -> web::Data<HealthState> {
        web::Data::new(HealthState::new())
    }

    #[rstest]
    #[actix_web::test]
    async fn readiness_returns_unavailable_until_ready(health_state: web::Data<HealthState>) {
        let app = test::init_service(
            App::new()
                .app_data(health_state.clone())
                .service(ready)
                .service(live),
        )
        .await;
        let request = test::TestRequest::get().uri("/health/ready").to_request();

        let response = test::call_service(&app, request).await;

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL),
            Some(&header::HeaderValue::from_static("no-store"))
        );
    }

    #[rstest]
    #[actix_web::test]
    async fn readiness_returns_ok_after_ready(health_state: web::Data<HealthState>) {
        health_state.mark_ready();
        let app = test::init_service(App::new().app_data(health_state).service(ready)).await;
        let request = test::TestRequest::get().uri("/health/ready").to_request();

        let response = test::call_service(&app, request).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL),
            Some(&header::HeaderValue::from_static("no-store"))
        );
    }

    #[rstest]
    #[actix_web::test]
    async fn liveness_returns_ok_while_live(health_state: web::Data<HealthState>) {
        let app = test::init_service(App::new().app_data(health_state).service(live)).await;
        let request = test::TestRequest::get().uri("/health/live").to_request();

        let response = test::call_service(&app, request).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL),
            Some(&header::HeaderValue::from_static("no-store"))
        );
    }

    #[rstest]
    #[actix_web::test]
    async fn liveness_returns_unavailable_after_unhealthy(health_state: web::Data<HealthState>) {
        health_state.mark_unhealthy();
        let app = test::init_service(App::new().app_data(health_state).service(live)).await;
        let request = test::TestRequest::get().uri("/health/live").to_request();

        let response = test::call_service(&app, request).await;

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL),
            Some(&header::HeaderValue::from_static("no-store"))
        );
    }
}
