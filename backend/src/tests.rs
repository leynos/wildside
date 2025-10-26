//! Tests for the backend application bootstrap, covering metrics initialisation
//! and readiness signalling.

use super::{create_server, HealthState, ServerConfig};
#[cfg(feature = "metrics")]
use super::{initialize_metrics, PrometheusMetricsBuilder};
use actix_web::cookie::{Key, SameSite};
use actix_web::dev::{Server, ServerHandle};
use actix_web::web;
use rstest::{fixture, rstest};
use std::net::SocketAddr;

#[fixture]
fn health_state() -> web::Data<HealthState> {
    web::Data::new(HealthState::new())
}

#[fixture]
fn session_key() -> Key {
    Key::generate()
}

#[fixture]
fn bind_addr() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], 0))
}

#[fixture]
fn cookie_secure() -> bool {
    false
}

#[fixture]
fn same_site_policy() -> SameSite {
    SameSite::Lax
}

#[rstest]
fn server_config_bind_addr_round_trips(session_key: Key, bind_addr: SocketAddr) {
    let config = ServerConfig::new(session_key, true, SameSite::Strict, bind_addr);
    assert_eq!(config.bind_addr(), bind_addr);
}

#[cfg(feature = "metrics")]
#[rstest]
fn server_config_metrics_default_to_none(
    session_key: Key,
    bind_addr: SocketAddr,
    cookie_secure: bool,
    same_site_policy: SameSite,
) {
    let config = ServerConfig::new(session_key, cookie_secure, same_site_policy, bind_addr);
    assert!(
        config.metrics().is_none(),
        "expected metrics to default to None"
    );
}

#[cfg(feature = "metrics")]
#[rstest]
fn server_config_with_metrics_preserves_value(
    session_key: Key,
    bind_addr: SocketAddr,
    cookie_secure: bool,
    same_site_policy: SameSite,
) {
    let metrics = PrometheusMetricsBuilder::new("test")
        .endpoint("/metrics")
        .build()
        .expect("metrics should be buildable in tests");
    let config = ServerConfig::new(session_key, cookie_secure, same_site_policy, bind_addr)
        .with_metrics(Some(metrics));

    assert!(
        config.metrics().is_some(),
        "metrics helper should retain provided value"
    );
}

#[fixture]
fn server_config(
    session_key: Key,
    bind_addr: SocketAddr,
    cookie_secure: bool,
    same_site_policy: SameSite,
) -> ServerConfig {
    ServerConfig::new(session_key, cookie_secure, same_site_policy, bind_addr)
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
) -> (Server, ServerHandle) {
    assert!(!health_state.is_ready(), "state should start unready");

    let server = create_server(health_state.clone(), server_config)
        .expect("server should build from configuration");
    let handle = server.handle();

    assert!(
        health_state.is_ready(),
        "server creation should mark readiness"
    );
    (server, handle)
}

#[cfg(feature = "metrics")]
#[rstest]
#[actix_rt::test]
async fn create_server_marks_ready_without_metrics(
    health_state: web::Data<HealthState>,
    server_config: ServerConfig,
) {
    let config = server_config.with_metrics(None);
    let (server, handle) = assert_server_marks_ready(health_state, config).await;
    drop(handle.stop(true));
    drop(handle);
    drop(server);
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
    let (server, handle) = assert_server_marks_ready(health_state, config).await;
    drop(handle.stop(true));
    drop(handle);
    drop(server);
}

#[cfg(not(feature = "metrics"))]
#[rstest]
#[actix_rt::test]
async fn create_server_marks_ready_non_metrics_build(
    health_state: web::Data<HealthState>,
    server_config: ServerConfig,
) {
    let (server, handle) = assert_server_marks_ready(health_state, server_config).await;
    drop(handle.stop(true));
    drop(handle);
    drop(server);
}
