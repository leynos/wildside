//! Behaviour tests for externally observable health probes.

use actix_web::http::StatusCode;
use actix_web::http::header::{CACHE_CONTROL, HeaderValue};
use actix_web::{App, test, web};
use backend::inbound::http::health::{HealthState, live, ready};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::RefCell;

struct HealthProbeWorld {
    health: web::Data<HealthState>,
    response: RefCell<Option<ProbeResponse>>,
}

struct ProbeResponse {
    status: StatusCode,
    cache_control: Option<HeaderValue>,
}

impl HealthProbeWorld {
    fn new() -> Self {
        Self {
            health: web::Data::new(HealthState::new()),
            response: RefCell::new(None),
        }
    }

    async fn request_probe(&self, uri: &str) {
        let app = test::init_service(
            App::new()
                .app_data(self.health.clone())
                .service(ready)
                .service(live),
        )
        .await;
        let request = test::TestRequest::get().uri(uri).to_request();
        let response = test::call_service(&app, request).await;
        let response = ProbeResponse {
            status: response.status(),
            cache_control: response.headers().get(CACHE_CONTROL).cloned(),
        };
        *self.response.borrow_mut() = Some(response);
    }

    async fn request_readiness(&self) {
        self.request_probe("/health/ready").await;
    }

    async fn request_liveness(&self) {
        self.request_probe("/health/live").await;
    }

    fn with_response<F>(&self, f: F)
    where
        F: FnOnce(&ProbeResponse),
    {
        let response = self.response.borrow();
        let response = response.as_ref().expect("probe response");
        f(response);
    }
}

#[fixture]
fn world() -> HealthProbeWorld {
    HealthProbeWorld::new()
}

#[given("a live Wildside runtime")]
fn live_runtime(world: &HealthProbeWorld) {
    let _ = world;
}

#[given("the runtime is ready")]
fn runtime_is_ready(world: &HealthProbeWorld) {
    world.health.mark_ready();
}

#[given("the runtime is unhealthy")]
fn runtime_is_unhealthy(world: &HealthProbeWorld) {
    world.health.mark_unhealthy();
}

#[when("the readiness probe is requested")]
async fn readiness_probe_is_requested(world: &HealthProbeWorld) {
    world.request_readiness().await;
}

#[when("the liveness probe is requested")]
async fn liveness_probe_is_requested(world: &HealthProbeWorld) {
    world.request_liveness().await;
}

#[then("the probe response status is {status}")]
fn probe_response_status_is(world: &HealthProbeWorld, status: u16) {
    let expected = StatusCode::from_u16(status).expect("valid feature status");
    world.with_response(|response| assert_eq!(response.status, expected));
}

#[then("the probe response is not cacheable")]
fn probe_response_is_not_cacheable(world: &HealthProbeWorld) {
    world.with_response(|response| {
        assert_eq!(
            response.cache_control.as_ref(),
            Some(&HeaderValue::from_static("no-store"))
        );
    });
}

#[scenario(path = "tests/features/health_probes.feature")]
#[tokio::test(flavor = "current_thread")]
async fn health_probe_scenarios(world: HealthProbeWorld) {
    drop(world);
}
