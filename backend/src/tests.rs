//! Tests for the backend application bootstrap, covering metrics initialisation
//! and readiness signalling.

#[cfg(feature = "metrics")]
use super::initialize_metrics;
use super::{create_server, HealthState, ServerConfig};
use actix_web::cookie::{Key, SameSite};
use actix_web::web;
#[cfg(feature = "metrics")]
use actix_web_prom::PrometheusMetricsBuilder;
use rstest::{fixture, rstest};
use std::net::SocketAddr;
use tokio::time::{timeout, Duration};

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
fn server_config_bind_addr_round_trips(server_config: ServerConfig, bind_addr: SocketAddr) {
    assert_eq!(
        server_config.bind_addr(),
        bind_addr,
        "bind_addr should round-trip through ServerConfig"
    );
}

#[cfg(feature = "metrics")]
#[rstest]
fn server_config_metrics_default_to_none(server_config: ServerConfig) {
    assert!(
        server_config.metrics().is_none(),
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

async fn assert_server_marks_ready(
    health_state: web::Data<HealthState>,
    server_config: ServerConfig,
) {
    assert!(!health_state.is_ready(), "state should start unready");

    let server = create_server(health_state.clone(), server_config)
        .expect("server should build from configuration");
    let handle = server.handle();
    let server_join = actix_rt::spawn(server);

    assert!(
        health_state.is_ready(),
        "server creation should mark readiness"
    );
    timeout(Duration::from_secs(5), handle.stop(true))
        .await
        .expect("timed out waiting for server.stop");
    let join_result = timeout(Duration::from_secs(5), server_join)
        .await
        .expect("timed out waiting for server task join")
        .expect("server task should not panic");
    join_result.expect("server should stop without IO errors");
}

#[cfg(feature = "metrics")]
#[rstest]
#[case(false)]
#[case(true)]
#[actix_rt::test]
async fn create_server_marks_ready_with_optional_metrics(
    health_state: web::Data<HealthState>,
    server_config: ServerConfig,
    #[case] with_metrics: bool,
) {
    let config = if with_metrics {
        let metrics = PrometheusMetricsBuilder::new("test")
            .endpoint("/metrics")
            .build()
            .expect("metrics should build in tests");
        server_config.with_metrics(Some(metrics))
    } else {
        server_config.with_metrics(None)
    };
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
