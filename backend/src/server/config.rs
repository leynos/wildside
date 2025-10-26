//! HTTP server configuration object and helpers.

use actix_web::cookie::{Key, SameSite};
use std::net::SocketAddr;

#[cfg(feature = "metrics")]
use actix_web_prom::PrometheusMetrics;

/// Builder-style configuration for creating the HTTP server.
pub struct ServerConfig {
    pub(crate) key: Key,
    pub(crate) cookie_secure: bool,
    pub(crate) same_site: SameSite,
    pub(crate) bind_addr: SocketAddr,
    #[cfg(feature = "metrics")]
    pub(crate) prometheus: Option<PrometheusMetrics>,
}

impl ServerConfig {
    /// Construct a server configuration using application preferences.
    #[must_use]
    pub fn new(key: Key, cookie_secure: bool, same_site: SameSite, bind_addr: SocketAddr) -> Self {
        Self {
            key,
            cookie_secure,
            same_site,
            bind_addr,
            #[cfg(feature = "metrics")]
            prometheus: None,
        }
    }

    /// Return the socket address the server will bind to.
    #[cfg_attr(not(any(test, doctest)), allow(dead_code))]
    #[must_use]
    pub fn bind_addr(&self) -> SocketAddr {
        self.bind_addr
    }

    #[cfg(feature = "metrics")]
    /// Attach Prometheus middleware to the configuration.
    #[must_use]
    pub fn with_metrics(mut self, prometheus: Option<PrometheusMetrics>) -> Self {
        self.prometheus = prometheus;
        self
    }

    #[cfg(feature = "metrics")]
    /// Return the configured Prometheus middleware, if any.
    #[cfg_attr(not(any(test, doctest)), allow(dead_code))]
    #[must_use]
    pub fn metrics(&self) -> Option<&PrometheusMetrics> {
        self.prometheus.as_ref()
    }
}
