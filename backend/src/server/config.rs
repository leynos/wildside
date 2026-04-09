//! HTTP server configuration object and helpers.

use actix_web::cookie::{Key, SameSite};
use backend::outbound::persistence::DbPool;
use std::net::SocketAddr;

#[cfg(feature = "metrics")]
use actix_web_prom::PrometheusMetrics;

/// Builder-style configuration for creating the HTTP server.
pub struct ServerConfig {
    #[cfg_attr(test, allow(dead_code))]
    pub(crate) key: Key,
    #[cfg_attr(test, allow(dead_code))]
    pub(crate) cookie_secure: bool,
    #[cfg_attr(test, allow(dead_code))]
    pub(crate) same_site: SameSite,
    #[cfg_attr(test, allow(dead_code))]
    pub(crate) bind_addr: SocketAddr,
    pub(crate) db_pool: Option<DbPool>,
    #[cfg(feature = "metrics")]
    #[cfg_attr(test, allow(dead_code))]
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
            db_pool: None,
            #[cfg(feature = "metrics")]
            prometheus: None,
        }
    }

    /// Attach a database connection pool for persistence adapters.
    ///
    /// When provided, the server will use database-backed implementations
    /// for ports that have adapters available (e.g., `RouteSubmissionService`).
    #[must_use]
    #[cfg_attr(test, allow(dead_code))]
    pub fn with_db_pool(mut self, pool: DbPool) -> Self {
        self.db_pool = Some(pool);
        self
    }

    /// Return the socket address the server will bind to.
    #[must_use]
    pub fn bind_addr(&self) -> SocketAddr {
        self.bind_addr
    }

    #[cfg(feature = "metrics")]
    /// Attach Prometheus middleware to the configuration.
    #[must_use]
    #[cfg_attr(test, allow(dead_code))]
    pub fn with_metrics(mut self, prometheus: Option<PrometheusMetrics>) -> Self {
        self.prometheus = prometheus;
        self
    }

    #[cfg(feature = "metrics")]
    /// Return the configured Prometheus middleware, if any.
    #[must_use]
    pub fn metrics(&self) -> Option<&PrometheusMetrics> {
        self.prometheus.as_ref()
    }
}
