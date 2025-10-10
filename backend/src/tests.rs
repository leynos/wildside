//! Tests for the backend application bootstrap.

use super::*;
use actix_web::cookie::SameSite;

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
#[actix_rt::test]
async fn create_server_marks_ready_without_metrics() {
    let state = web::Data::new(HealthState::new());
    assert!(!state.is_ready(), "state should start unready");

    let server = create_server(
        state.clone(),
        Key::generate(),
        false,
        SameSite::Lax,
        ("127.0.0.1".into(), 0),
        None,
    )
    .expect("server should build without metrics");

    assert!(state.is_ready(), "server creation should mark readiness");
    drop(server);
}

#[cfg(feature = "metrics")]
#[actix_rt::test]
async fn create_server_marks_ready_with_metrics() {
    let state = web::Data::new(HealthState::new());
    assert!(!state.is_ready(), "state should start unready");

    let prometheus = PrometheusMetricsBuilder::new("test")
        .endpoint("/metrics")
        .build()
        .expect("metrics should build for tests");

    let server = create_server(
        state.clone(),
        Key::generate(),
        false,
        SameSite::Lax,
        ("127.0.0.1".into(), 0),
        Some(prometheus),
    )
    .expect("server should build with metrics");

    assert!(state.is_ready(), "server creation should mark readiness");
    drop(server);
}

#[cfg(not(feature = "metrics"))]
#[actix_rt::test]
async fn create_server_marks_ready_without_metrics() {
    let state = web::Data::new(HealthState::new());
    assert!(!state.is_ready(), "state should start unready");

    let server = create_server(
        state.clone(),
        Key::generate(),
        false,
        SameSite::Lax,
        ("127.0.0.1".into(), 0),
    )
    .expect("server should build without metrics");

    assert!(state.is_ready(), "server creation should mark readiness");
    drop(server);
}
