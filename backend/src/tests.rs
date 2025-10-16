//! Tests for the backend application bootstrap, covering metrics initialisation
//! and readiness signalling.

use super::{create_server, HealthState};
#[cfg(feature = "metrics")]
use super::{initialize_metrics, PrometheusMetricsBuilder};
use actix_web::cookie::{Key, SameSite};
use actix_web::web;
use rstest::{fixture, rstest};

#[fixture]
fn health_state() -> web::Data<HealthState> {
    web::Data::new(HealthState::new())
}

#[fixture]
fn session_key() -> Key {
    Key::generate()
}

#[fixture]
fn bind_address() -> (String, u16) {
    ("127.0.0.1".into(), 0)
}

#[fixture]
fn cookie_secure() -> bool {
    false
}

#[fixture]
fn same_site_policy() -> SameSite {
    SameSite::Lax
}

#[cfg(feature = "metrics")]
#[test]
fn initialize_metrics_returns_none_on_error() {
    let metrics = initialize_metrics(|| -> Result<_, &str> { Err("boom") });
    assert!(metrics.is_none(), "expected metrics to be absent on error");
}

#[cfg(feature = "metrics")]
#[test]
fn initialize_metrics_returns_metrics_on_success() {
    let metrics = initialize_metrics(|| {
        PrometheusMetricsBuilder::new("test")
            .endpoint("/metrics")
            .build()
    });

    assert!(
        metrics.is_some(),
        "expected metrics to be present on success"
    );
}

#[cfg(feature = "metrics")]
#[fixture]
fn prometheus_metrics() -> actix_web_prom::PrometheusMetrics {
    PrometheusMetricsBuilder::new("test")
        .endpoint("/metrics")
        .build()
        .expect("metrics should build for tests")
}

#[cfg(feature = "metrics")]
#[rstest]
#[actix_rt::test]
async fn create_server_marks_ready_without_metrics(
    health_state: web::Data<HealthState>,
    session_key: Key,
    bind_address: (String, u16),
    cookie_secure: bool,
    same_site_policy: SameSite,
) {
    assert!(!health_state.is_ready(), "state should start unready");

    let _server = create_server(
        health_state.clone(),
        session_key,
        cookie_secure,
        same_site_policy,
        bind_address,
        None,
    )
    .expect("server should build without metrics");

    assert!(
        health_state.is_ready(),
        "server creation should mark readiness"
    );
}

#[cfg(feature = "metrics")]
#[rstest]
#[actix_rt::test]
async fn create_server_marks_ready_with_metrics(
    health_state: web::Data<HealthState>,
    session_key: Key,
    bind_address: (String, u16),
    prometheus_metrics: actix_web_prom::PrometheusMetrics,
    cookie_secure: bool,
    same_site_policy: SameSite,
) {
    assert!(!health_state.is_ready(), "state should start unready");

    let _server = create_server(
        health_state.clone(),
        session_key,
        cookie_secure,
        same_site_policy,
        bind_address,
        Some(prometheus_metrics),
    )
    .expect("server should build with metrics");

    assert!(
        health_state.is_ready(),
        "server creation should mark readiness"
    );
}

#[cfg(not(feature = "metrics"))]
#[rstest]
#[actix_rt::test]
async fn create_server_marks_ready_non_metrics_build(
    health_state: web::Data<HealthState>,
    session_key: Key,
    bind_address: (String, u16),
    cookie_secure: bool,
    same_site_policy: SameSite,
) {
    assert!(!health_state.is_ready(), "state should start unready");

    let _server = create_server(
        health_state.clone(),
        session_key,
        cookie_secure,
        same_site_policy,
        bind_address,
    )
    .expect("server should build without metrics");

    assert!(
        health_state.is_ready(),
        "server creation should mark readiness"
    );
}
