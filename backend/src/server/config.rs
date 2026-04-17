//! HTTP server configuration object and helpers.

use actix_web::cookie::{Key, SameSite};
use backend::outbound::persistence::DbPool;
use std::net::SocketAddr;

#[cfg(feature = "metrics")]
use actix_web_prom::PrometheusMetrics;

/// Builder-style configuration for creating the HTTP server.
pub struct ServerConfig {
    pub(crate) key: Key,
    pub(crate) cookie_secure: bool,
    pub(crate) same_site: SameSite,
    pub(crate) bind_addr: SocketAddr,
    pub(crate) db_pool: Option<DbPool>,
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
    pub fn with_db_pool(mut self, pool: DbPool) -> Self {
        self.db_pool = Some(pool);
        self
    }

    /// Return the socket address the server will bind to.
    #[cfg(test)]
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
    #[must_use]
    pub fn metrics(&self) -> Option<&PrometheusMetrics> {
        self.prometheus.as_ref()
    }
}

#[cfg(test)]
impl ServerConfig {
    /// Return the session signing key (test-only accessor).
    pub(crate) fn key(&self) -> &Key {
        &self.key
    }

    /// Return whether the session cookie is Secure-flagged (test-only accessor).
    pub(crate) fn cookie_secure(&self) -> bool {
        self.cookie_secure
    }

    /// Return the SameSite policy (test-only accessor).
    pub(crate) fn same_site(&self) -> SameSite {
        self.same_site
    }

    #[cfg(feature = "metrics")]
    /// Return the optional Prometheus middleware (test-only accessor).
    pub(crate) fn prometheus_ref(&self) -> Option<&PrometheusMetrics> {
        self.prometheus.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::cookie::SameSite;
    use std::net::SocketAddr;

    /// Exercises every builder method and field accessor so that the dead-code
    /// lint cannot fire in any compilation unit that includes this file.
    #[test]
    fn server_config_all_methods_reachable() {
        let addr: SocketAddr = "127.0.0.1:0".parse().expect("valid loopback address");
        let cfg = ServerConfig::new(Key::generate(), true, SameSite::Strict, addr);

        assert_eq!(cfg.bind_addr(), addr);
        assert!(cfg.cookie_secure());
        assert_eq!(cfg.same_site(), SameSite::Strict);
        let _ = cfg.key();

        #[cfg(feature = "metrics")]
        {
            assert!(cfg.metrics().is_none());
            assert!(cfg.prometheus_ref().is_none());
            let cfg2 =
                ServerConfig::new(Key::generate(), false, SameSite::Lax, addr).with_metrics(None);
            assert!(cfg2.metrics().is_none());
        }
    }

    fn _with_db_pool_is_callable(cfg: ServerConfig, pool: DbPool) {
        let _ = cfg.with_db_pool(pool);
    }
}
