#![cfg_attr(not(any(test, doctest)), deny(clippy::unwrap_used))]
// Keep unwrap banned; allow `expect` so call sites can document assumptions.
//! Backend entry-point: wires REST endpoints, WebSocket entry, and OpenAPI docs.

use actix_web::web;
#[cfg(feature = "metrics")]
use actix_web_prom::PrometheusMetricsBuilder;
use backend::inbound::http::session_config::{session_settings_from_env, BuildMode, DefaultEnv};
use std::env;
use std::net::SocketAddr;
use tracing::warn;
use tracing_subscriber::{fmt, EnvFilter};

use backend::inbound::http::health::HealthState;

mod server;

use server::{create_server, ServerConfig};

#[cfg(feature = "metrics")]
fn make_metrics(
) -> Result<actix_web_prom::PrometheusMetrics, Box<dyn std::error::Error + Send + Sync>> {
    PrometheusMetricsBuilder::new("wildside")
        .endpoint("/metrics")
        .build()
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

fn parse_port_with_fallback(port_str: &str) -> u16 {
    match port_str.parse::<u16>() {
        Ok(port) => port,
        Err(_) => {
            warn!(value = %port_str, "invalid PORT; falling back to 8080");
            8080u16
        }
    }
}

fn bind_addr() -> SocketAddr {
    let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port = env::var("PORT")
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
    let cookie_secure = session_settings.cookie_secure;
    let same_site = session_settings.same_site;
    let key = session_settings.key;
    #[cfg(feature = "metrics")]
    let prometheus = initialize_metrics(make_metrics);
    let health_state = web::Data::new(HealthState::new());
    let server_config = {
        let config = ServerConfig::new(key, cookie_secure, same_site, bind_addr());
        #[cfg(feature = "metrics")]
        let config = config.with_metrics(prometheus);
        config
    };
    let server = create_server(health_state.clone(), server_config)?;
    server.await
}

#[cfg(test)]
mod tests;
