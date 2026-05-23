//! Behaviour tests for externally observable health probes.

use actix_web::http::StatusCode;
use actix_web::http::header::{CACHE_CONTROL, CONTENT_TYPE, HeaderValue};
use actix_web::{App, test, web};
use backend::inbound::http::health::{HealthState, live, ready};
use insta::assert_json_snapshot;
use rstest::{fixture, rstest};
use rstest_bdd_macros::{given, scenario, then, when};
use serde_json::{Value, json};
use std::cell::RefCell;

struct HealthProbeWorld {
    health: web::Data<HealthState>,
    response: RefCell<Option<ProbeResponse>>,
}

struct ProbeResponse {
    uri: &'static str,
    status: StatusCode,
    cache_control: Option<HeaderValue>,
    content_type: Option<HeaderValue>,
    body: Value,
}

#[derive(Clone, Copy)]
enum ProbeSetup {
    Default,
    Ready,
    Unhealthy,
}

impl HealthProbeWorld {
    fn new() -> Self {
        Self {
            health: web::Data::new(HealthState::new()),
            response: RefCell::new(None),
        }
    }

    async fn request_probe(&self, uri: &'static str) {
        let app = test::init_service(
            App::new()
                .app_data(self.health.clone())
                .service(ready)
                .service(live),
        )
        .await;
        let request = test::TestRequest::get().uri(uri).to_request();
        let response = test::call_service(&app, request).await;
        let status = response.status();
        let cache_control = response.headers().get(CACHE_CONTROL).cloned();
        let content_type = response.headers().get(CONTENT_TYPE).cloned();
        let body = test::read_body(response).await;
        let body = serde_json::from_slice(body.as_ref()).expect("health probe JSON body");
        let response = ProbeResponse {
            uri,
            status,
            cache_control,
            content_type,
            body,
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

impl ProbeResponse {
    fn snapshot_name(&self) -> String {
        let uri = self.uri.trim_start_matches('/').replace('/', "_");
        format!("health_probe_{uri}_{}", self.status.as_u16())
    }

    fn snapshot_payload(&self) -> Value {
        json!({
            "uri": self.uri,
            "status": self.status.as_u16(),
            "headers": {
                "cache-control": header_value_to_str(&self.cache_control),
                "content-type": header_value_to_str(&self.content_type),
            },
            "body": self.body,
        })
    }
}

fn header_value_to_str(header: &Option<HeaderValue>) -> Option<&str> {
    header
        .as_ref()
        .map(|value| value.to_str().expect("health probe header value"))
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

#[rstest]
#[case::readiness_not_ready("/health/ready", ProbeSetup::Default)]
#[case::readiness_ready("/health/ready", ProbeSetup::Ready)]
#[case::liveness_live("/health/live", ProbeSetup::Default)]
#[case::liveness_unhealthy("/health/live", ProbeSetup::Unhealthy)]
#[tokio::test(flavor = "current_thread")]
async fn health_probe_responses_match_snapshots(
    #[case] uri: &'static str,
    #[case] setup: ProbeSetup,
) {
    let world = HealthProbeWorld::new();
    match setup {
        ProbeSetup::Default => {}
        ProbeSetup::Ready => world.health.mark_ready(),
        ProbeSetup::Unhealthy => world.health.mark_unhealthy(),
    }

    world.request_probe(uri).await;

    world.with_response(|response| {
        assert_json_snapshot!(response.snapshot_name(), response.snapshot_payload());
    });
}

#[scenario(path = "tests/features/health_probes.feature")]
#[tokio::test(flavor = "current_thread")]
async fn health_probe_scenarios(world: HealthProbeWorld) {
    drop(world);
}
