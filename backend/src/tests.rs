//! Tests for the backend application bootstrap, covering metrics initialization
//! and readiness signalling.

#[cfg(feature = "metrics")]
use super::initialize_metrics;
use super::{
    HealthState, ServerConfig, bind_addr as resolve_bind_addr, build_db_pool, create_server,
};
use actix_web::cookie::{Key, SameSite};
use actix_web::web;
#[cfg(feature = "metrics")]
use actix_web_prom::PrometheusMetricsBuilder;
use backend::domain::ports::HealthObserver;
use rstest::{fixture, rstest};
#[cfg(feature = "metrics")]
use std::error::Error;
use std::net::SocketAddr;
use tokio::time::{Duration, timeout};

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
    let metrics = match PrometheusMetricsBuilder::new("test")
        .endpoint("/metrics")
        .build()
    {
        Ok(metrics) => metrics,
        Err(error) => panic!("metrics should be buildable in tests: {error}"),
    };
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
    assert!(
        !health_state.observe_readiness().is_healthy(),
        "state should start unready"
    );

    let server = match create_server(health_state.clone(), server_config) {
        Ok(server) => server,
        Err(error) => panic!("server should build from configuration: {error}"),
    };
    let handle = server.handle();
    let server_join = actix_rt::spawn(server);

    assert!(
        health_state.observe_readiness().is_healthy(),
        "server creation should mark readiness"
    );
    if let Err(error) = timeout(Duration::from_secs(5), handle.stop(true)).await {
        panic!("timed out waiting for server.stop: {error}");
    }
    let join_result = match timeout(Duration::from_secs(5), server_join).await {
        Ok(join_result) => join_result,
        Err(error) => panic!("timed out waiting for server task join: {error}"),
    };
    let join_result = match join_result {
        Ok(join_result) => join_result,
        Err(error) => panic!("server task should not panic: {error}"),
    };
    if let Err(error) = join_result {
        panic!("server should stop without IO errors: {error}");
    }
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
        let metrics = match PrometheusMetricsBuilder::new("test")
            .endpoint("/metrics")
            .build()
        {
            Ok(metrics) => metrics,
            Err(error) => panic!("metrics should build in tests: {error}"),
        };
        server_config.with_metrics(Some(metrics))
    } else {
        server_config.with_metrics(None)
    };
    assert_server_marks_ready(health_state, config).await;
}

#[cfg(feature = "metrics")]
struct PageMetricsScrape {
    login_status: reqwest::StatusCode,
    users_status: reqwest::StatusCode,
    returned_rows: usize,
    metrics_status: reqwest::StatusCode,
    metrics_body: String,
}

#[cfg(feature = "metrics")]
async fn request_users_page_and_scrape(
    client: &reqwest::Client,
    base_url: &str,
) -> Result<PageMetricsScrape, Box<dyn Error>> {
    let login_response = client
        .post(format!("{base_url}/api/v1/login"))
        .json(&serde_json::json!({
            "username": "admin",
            "password": "password",
        }))
        .send()
        .await?;
    let login_status = login_response.status();
    let session_cookie = login_response
        .headers()
        .get(reqwest::header::SET_COOKIE)
        .ok_or_else(|| std::io::Error::other("login response omitted its session cookie"))?
        .to_str()?
        .split(';')
        .next()
        .ok_or_else(|| std::io::Error::other("session cookie had no cookie pair"))?
        .to_owned();

    let users_response = client
        .get(format!("{base_url}/api/v1/users"))
        .header(reqwest::header::COOKIE, session_cookie)
        .send()
        .await?;
    let users_status = users_response.status();
    let users_body: serde_json::Value = users_response.json().await?;
    let returned_rows = users_body
        .get("data")
        .and_then(serde_json::Value::as_array)
        .map(Vec::len)
        .ok_or_else(|| std::io::Error::other("users response omitted its data array"))?;

    let metrics_response = client.get(format!("{base_url}/metrics")).send().await?;
    Ok(PageMetricsScrape {
        login_status,
        users_status,
        returned_rows,
        metrics_status: metrics_response.status(),
        metrics_body: metrics_response.text().await?,
    })
}

#[cfg(feature = "metrics")]
async fn stop_test_server(
    server_handle: actix_web::dev::ServerHandle,
    server_join: tokio::task::JoinHandle<std::io::Result<()>>,
) {
    assert!(
        timeout(Duration::from_secs(5), server_handle.stop(true))
            .await
            .is_ok(),
        "server shutdown should complete within five seconds"
    );
    let join_result = match timeout(Duration::from_secs(5), server_join).await {
        Ok(join_result) => join_result,
        Err(error) => panic!("timed out waiting for pagination metrics server: {error}"),
    };
    match join_result {
        Ok(Ok(())) => {}
        Ok(Err(error)) => panic!("pagination metrics server should stop cleanly: {error}"),
        Err(error) => panic!("pagination metrics server task should not panic: {error}"),
    }
}

#[cfg(feature = "metrics")]
fn assert_page_metrics_scrape(scrape: &PageMetricsScrape) {
    assert_eq!(
        scrape.login_status.as_u16(),
        200,
        "fixture login should succeed"
    );
    assert_eq!(
        scrape.users_status.as_u16(),
        200,
        "users page request should succeed"
    );
    assert_eq!(
        scrape.returned_rows, 1,
        "fixture users response should return one row"
    );
    assert_eq!(
        scrape.metrics_status.as_u16(),
        200,
        "Prometheus scrape should succeed"
    );
    assert!(
        scrape.metrics_body.contains(
            "wildside_pagination_pages_total{endpoint=\"users\",limit_source=\"default\",outcome=\"terminal\",traversal=\"first\"} 1"
        ),
        "scrape should include one terminal first page with the default limit"
    );
    assert!(
        scrape.metrics_body.contains(
            "wildside_pagination_rows_returned_total{endpoint=\"users\",traversal=\"first\"} 1"
        ),
        "scrape should count the row returned by the fixture users page"
    );
}

/// Verify the ordinary server composition path records and exposes a user page.
#[cfg(feature = "metrics")]
#[actix_rt::test]
async fn create_server_scrape_contains_pagination_metrics_for_delivered_page() {
    let prometheus = PrometheusMetricsBuilder::new("test")
        .endpoint("/metrics")
        .build()
        .expect("Prometheus middleware should build in tests");
    let config = ServerConfig::new(
        Key::generate(),
        false,
        SameSite::Lax,
        SocketAddr::from(([127, 0, 0, 1], 0)),
    )
    .with_metrics(Some(prometheus));
    let health_state = web::Data::new(HealthState::new());
    let (server, server_address) = crate::server::create_server_with_address(health_state, config)
        .expect("server should build from metrics configuration");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("test HTTP client should have a bounded request timeout");
    let server_handle = server.handle();
    let server_join = actix_rt::spawn(server);
    let base_url = format!("http://{server_address}");

    let scrape = request_users_page_and_scrape(&client, &base_url).await;
    stop_test_server(server_handle, server_join).await;

    let scrape = scrape.expect("login, page request, and metrics scrape should complete");
    assert_page_metrics_scrape(&scrape);
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

/// Scenario: `HOST` and `PORT` are absent, valid, or unparsable in an
/// injected environment reader.
///
/// Invariant: the bind address falls back to `0.0.0.0:8080` for every input
/// the reader cannot turn into a socket address, and honours valid values.
/// The reader is a closure, so no case touches the process environment.
#[rstest]
#[case::defaults(None, None, "0.0.0.0:8080")]
#[case::host_and_port(Some("127.0.0.1"), Some("9000"), "127.0.0.1:9000")]
#[case::invalid_port_falls_back(Some("127.0.0.1"), Some("not-a-port"), "127.0.0.1:8080")]
#[case::invalid_host_falls_back(Some("not a host"), Some("9000"), "0.0.0.0:8080")]
fn bind_addr_resolves_from_the_injected_reader(
    #[case] host: Option<&'static str>,
    #[case] port: Option<&'static str>,
    #[case] expected: &str,
) {
    let resolved = resolve_bind_addr(|name| match name {
        "HOST" => host.map(str::to_owned),
        "PORT" => port.map(str::to_owned),
        _ => None,
    });

    assert_eq!(
        resolved,
        expected.parse::<SocketAddr>().expect("expected address"),
        "the bind address must be derived from the injected reader alone"
    );
}

/// Scenario: no database URL is supplied to the pool builder.
///
/// Invariant: the builder reports "no pool" rather than reading the process
/// environment to find one.
#[rstest]
#[actix_rt::test]
async fn build_db_pool_returns_none_without_a_url() {
    let pool = build_db_pool(None)
        .await
        .expect("absent URL is not an error");
    assert!(
        pool.is_none(),
        "an absent database URL must yield no pool rather than an ambient lookup"
    );
}
