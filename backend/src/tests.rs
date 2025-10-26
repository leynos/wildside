//! Tests for the backend application bootstrap, covering metrics initialisation
//! and readiness signalling.

use super::{create_server, HealthState, ServerConfig};
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

#[test]
fn server_config_bind_address_round_trips() {
    let bind_address = ("127.0.0.1".into(), 8080);
    let config = ServerConfig::new(
        Key::generate(),
        true,
        SameSite::Strict,
        bind_address.clone(),
    );
    assert_eq!(config.bind_address(), &bind_address);
}

#[cfg(feature = "metrics")]
#[test]
fn server_config_metrics_default_to_none() {
    let config = ServerConfig::new(
        Key::generate(),
        false,
        SameSite::Lax,
        ("127.0.0.1".into(), 0),
    );
    assert!(
        config.metrics().is_none(),
        "expected metrics to default to None"
    );
}

#[cfg(feature = "metrics")]
#[test]
fn server_config_with_metrics_preserves_value() {
    let metrics = PrometheusMetricsBuilder::new("test")
        .endpoint("/metrics")
        .build()
        .expect("metrics should be buildable in tests");
    let config = ServerConfig::new(
        Key::generate(),
        false,
        SameSite::Lax,
        ("127.0.0.1".into(), 0),
    )
    .with_metrics(Some(metrics));

    assert!(
        config.metrics().is_some(),
        "metrics helper should retain provided value"
    );
}

#[fixture]
fn server_config(
    session_key: Key,
    bind_address: (String, u16),
    cookie_secure: bool,
    same_site_policy: SameSite,
) -> ServerConfig {
    ServerConfig::new(session_key, cookie_secure, same_site_policy, bind_address)
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

async fn assert_server_marks_ready(
    health_state: web::Data<HealthState>,
    server_config: ServerConfig,
) {
    assert!(!health_state.is_ready(), "state should start unready");

    let _server = create_server(health_state.clone(), server_config)
        .expect("server should build from configuration");

    assert!(
        health_state.is_ready(),
        "server creation should mark readiness"
    );
}

#[cfg(feature = "metrics")]
#[rstest]
#[actix_rt::test]
async fn create_server_marks_ready_without_metrics(
    health_state: web::Data<HealthState>,
    server_config: ServerConfig,
) {
    let config = server_config.with_metrics(None);
    assert_server_marks_ready(health_state, config).await;
}

#[cfg(feature = "metrics")]
#[rstest]
#[actix_rt::test]
async fn create_server_marks_ready_with_metrics(
    health_state: web::Data<HealthState>,
    server_config: ServerConfig,
    prometheus_metrics: actix_web_prom::PrometheusMetrics,
) {
    let config = server_config.with_metrics(Some(prometheus_metrics));
    assert_server_marks_ready(health_state, config).await;
}

#[cfg(not(feature = "metrics"))]
#[rstest]
#[actix_rt::test]
async fn create_server_marks_ready_non_metrics_build(
    health_state: web::Data<HealthState>,
    server_config: ServerConfig,
) {
    assert_server_marks_ready(health_state, server_config).await;
}
