#![cfg_attr(not(any(test, doctest)), deny(clippy::unwrap_used))]
// Keep unwrap banned; allow `expect` so call sites can document assumptions.
//! Backend entry-point: wires REST endpoints, WebSocket entry, and OpenAPI docs.

use actix_web::web;
#[cfg(feature = "metrics")]
use actix_web_prom::PrometheusMetricsBuilder;
#[cfg(feature = "example-data")]
use backend::example_data::{ExampleDataSettings, seed_example_data_on_startup};
use backend::inbound::http::session_config::{BuildMode, DefaultEnv, session_settings_from_env};
use backend::outbound::persistence::{DbPool, PoolConfig};
#[cfg(feature = "example-data")]
use ortho_config::OrthoConfig;
use std::env;
use std::net::SocketAddr;
use tracing::{info, warn};
use tracing_subscriber::{EnvFilter, fmt};

use backend::inbound::http::health::HealthState;

mod server;

use server::{ServerConfig, create_server};

#[cfg(feature = "metrics")]
fn make_metrics()
-> Result<actix_web_prom::PrometheusMetrics, Box<dyn std::error::Error + Send + Sync>> {
    let metrics = PrometheusMetricsBuilder::new("wildside")
        .endpoint("/metrics")
        .build()?;
    backend::register_pagination_error_metrics(&metrics.registry)?;
    Ok(metrics)
}

#[cfg(feature = "metrics")]
pub(crate) fn initialize_metrics<F, E>(make: F) -> Option<actix_web_prom::PrometheusMetrics>
where
    F: FnOnce() -> Result<actix_web_prom::PrometheusMetrics, E>,
    E: std::fmt::Display,
{
    match make() {
        Ok(metrics) => Some(metrics),
        Err(error) => {
            warn!(
                error = %error,
                "failed to initialize Prometheus metrics; continuing without metrics"
            );
            None
        }
    }
}

/// Parse a `PORT` value, warning and falling back to 8080 when it is not a
/// valid port number.
fn parse_port_with_fallback(port_str: &str) -> u16 {
    match port_str.parse::<u16>() {
        Ok(port) => port,
        Err(_) => {
            warn!(value = %port_str, "invalid PORT; falling back to 8080");
            8080u16
        }
    }
}

/// Reads one variable from the process environment.
///
/// `main` is the composition root, so this is the binary's single sanctioned
/// ambient read. Every helper below takes the reader as an argument, which is
/// what lets `bind_addr` be unit-tested without touching the process.
#[expect(
    clippy::disallowed_methods,
    reason = "composition root: the binary reads its own process environment \
              once and injects the reader into the helpers below"
)]
fn process_env(name: &str) -> Option<String> {
    env::var(name).ok()
}

/// Resolve the listen address from `HOST` and `PORT` read through `read_env`.
///
/// Any combination the reader supplies that does not parse as a socket address
/// falls back to `0.0.0.0:8080` with a warning.
fn bind_addr(read_env: impl Fn(&str) -> Option<String>) -> SocketAddr {
    let host = read_env("HOST").unwrap_or_else(|| "0.0.0.0".into());
    let port = read_env("PORT")
        .as_deref()
        .map(parse_port_with_fallback)
        .unwrap_or(8080u16);

    let candidate = format!("{host}:{port}");
    match candidate.parse::<SocketAddr>() {
        Ok(addr) => addr,
        Err(error) => {
            warn!(address = %candidate, %error, "invalid HOST/PORT combination; falling back to 0.0.0.0:8080");
            SocketAddr::from(([0, 0, 0, 0], 8080))
        }
    }
}

/// Build the database pool from an already-resolved URL.
///
/// `None` means no database was configured, which is not an error; the server
/// runs without persistence.
///
/// # Errors
///
/// Returns an error when a URL was supplied but the pool cannot be created.
async fn build_db_pool(database_url: Option<String>) -> std::io::Result<Option<DbPool>> {
    let Some(database_url) = database_url else {
        return Ok(None);
    };

    let config = PoolConfig::new(database_url);
    DbPool::new(config).await.map(Some).map_err(|error| {
        std::io::Error::other(format!("database pool initialization failed: {error}"))
    })
}

/// Application bootstrap.
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    if let Err(e) = fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .json()
        .try_init()
    {
        warn!(error = %e, "tracing init failed");
    }

    let session_env = DefaultEnv::new();
    let session_settings =
        session_settings_from_env(&session_env, BuildMode::from_debug_assertions())
            .map_err(std::io::Error::other)?;
    info!(
        fingerprint = %session_settings.fingerprint,
        "session signing key loaded"
    );
    let cookie_secure = session_settings.cookie_secure;
    let same_site = session_settings.same_site;
    let key = session_settings.key;

    #[cfg(feature = "example-data")]
    let example_data_settings = ExampleDataSettings::load().map_err(std::io::Error::other)?;

    let db_pool = build_db_pool(process_env("DATABASE_URL")).await?;

    #[cfg(feature = "example-data")]
    seed_example_data_on_startup(&example_data_settings, db_pool.as_ref())
        .await
        .map_err(std::io::Error::other)?;
    #[cfg(feature = "metrics")]
    let prometheus = initialize_metrics(make_metrics);
    let health_state = web::Data::new(HealthState::new());
    let server_config = {
        let config = ServerConfig::new(key, cookie_secure, same_site, bind_addr(process_env));
        let config = if let Some(pool) = db_pool.clone() {
            config.with_db_pool(pool)
        } else {
            config
        };
        #[cfg(feature = "metrics")]
        let config = config.with_metrics(prometheus);
        config
    };
    let server = create_server(health_state.clone(), server_config)?;
    server.await
}

#[cfg(test)]
mod tests;
