#![cfg_attr(not(any(test, doctest)), deny(clippy::unwrap_used))]
// Keep unwrap banned; allow `expect` so call sites can document assumptions.
//! Backend entry-point: wires REST endpoints, WebSocket entry, and OpenAPI docs.

use actix_web::cookie::{Key, SameSite};
use actix_web::web;
#[cfg(feature = "metrics")]
use actix_web_prom::PrometheusMetricsBuilder;
use std::env;
use std::net::SocketAddr;
use tracing::warn;
use tracing_subscriber::{fmt, EnvFilter};
use zeroize::Zeroize;

use backend::api::health::HealthState;

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

fn load_session_key() -> std::io::Result<Key> {
    let key_path =
        env::var("SESSION_KEY_FILE").unwrap_or_else(|_| "/var/run/secrets/session_key".into());
    match std::fs::read(&key_path) {
        Ok(mut bytes) => {
            if !cfg!(debug_assertions) && bytes.len() < 64 {
                return Err(std::io::Error::other(format!(
                    "session key at {key_path} too short: need >=64 bytes, got {}",
                    bytes.len()
                )));
            }
            let key = Key::derive_from(&bytes);
            bytes.zeroize();
            Ok(key)
        }
        Err(e) => {
            let allow_dev = env::var("SESSION_ALLOW_EPHEMERAL").ok().as_deref() == Some("1");
            if cfg!(debug_assertions) || allow_dev {
                warn!(path = %key_path, error = %e, "using temporary session key (dev only)");
                Ok(Key::generate())
            } else {
                Err(std::io::Error::other(format!(
                    "failed to read session key at {key_path}: {e}"
                )))
            }
        }
    }
}

fn cookie_secure_from_env() -> bool {
    match env::var("SESSION_COOKIE_SECURE") {
        Ok(v) => match v.to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "y" => true,
            "0" | "false" | "no" | "n" => false,
            other => {
                warn!(value = %other, "invalid SESSION_COOKIE_SECURE; defaulting to secure");
                true
            }
        },
        Err(_) => true,
    }
}

/// Determine the session SameSite policy, allowing an environment override.
///
/// Defaults to `Lax` in debug builds and `Strict` otherwise. `SESSION_SAMESITE`
/// can set `Strict`, `Lax`, or `None`; choosing `None` requires a secure cookie
/// and some browsers may block such third-party cookies entirely.
fn same_site_from_env(cookie_secure: bool) -> std::io::Result<SameSite> {
    let default_same_site = if cfg!(debug_assertions) {
        SameSite::Lax
    } else {
        SameSite::Strict
    };
    Ok(match env::var("SESSION_SAMESITE") {
        Ok(v) => match v.to_ascii_lowercase().as_str() {
            "lax" => SameSite::Lax,
            "strict" => SameSite::Strict,
            "none" => {
                if !cookie_secure && !cfg!(debug_assertions) {
                    return Err(std::io::Error::other(
                        "SESSION_SAMESITE=None requires SESSION_COOKIE_SECURE=1",
                    ));
                }
                SameSite::None
            }
            other => {
                if cfg!(debug_assertions) {
                    warn!(value = %other, "invalid SESSION_SAMESITE, using default");
                    default_same_site
                } else {
                    return Err(std::io::Error::other(format!(
                        "invalid SESSION_SAMESITE: {other}"
                    )));
                }
            }
        },
        Err(_) => default_same_site,
    })
}

fn bind_addr() -> SocketAddr {
    let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port = match env::var("PORT") {
        Ok(p) => match p.parse::<u16>() {
            Ok(n) => n,
            Err(_) => {
                warn!(value = %p, "invalid PORT; falling back to 8080");
                8080u16
            }
        },
        Err(_) => 8080u16,
    };

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

    let key = load_session_key()?;
    let cookie_secure = cookie_secure_from_env();
    let same_site = same_site_from_env(cookie_secure)?;
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
