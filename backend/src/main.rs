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

async fn build_db_pool(seeding_enabled: bool) -> std::io::Result<Option<DbPool>> {
    let Ok(database_url) = env::var("DATABASE_URL") else {
        return Ok(None);
    };

    let config = PoolConfig::new(database_url);
    match DbPool::new(config).await {
        Ok(pool) => Ok(Some(pool)),
        Err(error) => {
            if seeding_enabled {
                Err(std::io::Error::other(format!(
                    "database pool initialization failed: {error}"
                )))
            } else {
                warn!(
                    error = %error,
                    "failed to initialize database pool; continuing without persistence"
                );
                Ok(None)
            }
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
    info!(
        fingerprint = %session_settings.fingerprint,
        "session signing key loaded"
    );
    let cookie_secure = session_settings.cookie_secure;
    let same_site = session_settings.same_site;
    let key = session_settings.key;

    #[cfg(feature = "example-data")]
    let example_data_settings = ExampleDataSettings::load().map_err(std::io::Error::other)?;
    #[cfg(feature = "example-data")]
    let seeding_enabled = example_data_settings.enabled;
    #[cfg(not(feature = "example-data"))]
    let seeding_enabled = false;

    let db_pool = build_db_pool(seeding_enabled).await?;

    #[cfg(feature = "example-data")]
    seed_example_data_on_startup(&example_data_settings, db_pool.as_ref())
        .await
        .map_err(std::io::Error::other)?;
    #[cfg(feature = "metrics")]
    let prometheus = initialize_metrics(make_metrics);
    let health_state = web::Data::new(HealthState::new());
    let server_config = {
        let config = ServerConfig::new(key, cookie_secure, same_site, bind_addr());
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
