//! Async-safe connection pool for Diesel PostgreSQL connections.
//!
//! This module wraps `diesel-async` and `bb8` to provide an ergonomic async
//! connection pool for the persistence layer. The pool manages connection
//! lifecycle, validation, and checkout with configurable limits.
//!
//! # Design
//!
//! - Uses `diesel-async`'s native async support rather than `spawn_blocking`
//! - Pool checkout is non-blocking and respects timeout configuration
//! - Connections are validated before use to detect stale connections
//! - All errors are mapped to domain-level `PoolError` variants

use std::time::Duration;

use diesel_async::pooled_connection::bb8::{Pool, PooledConnection};
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::AsyncPgConnection;

/// Errors that can occur during pool operations.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PoolError {
    /// Failed to check out a connection from the pool.
    #[error("failed to get connection from pool: {message}")]
    Checkout { message: String },

    /// Failed to build the connection pool.
    #[error("failed to build connection pool: {message}")]
    Build { message: String },
}

impl PoolError {
    /// Create a checkout error with the given message.
    pub fn checkout(message: impl Into<String>) -> Self {
        Self::Checkout {
            message: message.into(),
        }
    }

    /// Create a build error with the given message.
    pub fn build(message: impl Into<String>) -> Self {
        Self::Build {
            message: message.into(),
        }
    }
}

/// Configuration for the database connection pool.
///
/// # Example
///
/// ```ignore
/// let config = PoolConfig::new("postgres://user:pass@localhost/db")
///     .with_max_size(20)
///     .with_min_idle(Some(5))
///     .with_connection_timeout(Duration::from_secs(30));
/// ```
#[derive(Debug, Clone)]
pub struct PoolConfig {
    database_url: String,
    max_size: u32,
    min_idle: Option<u32>,
    connection_timeout: Duration,
}

impl PoolConfig {
    /// Create a new configuration with the given database URL.
    ///
    /// Uses sensible defaults:
    /// - `max_size`: 10 connections
    /// - `min_idle`: 2 connections
    /// - `connection_timeout`: 30 seconds
    pub fn new(database_url: impl Into<String>) -> Self {
        Self {
            database_url: database_url.into(),
            max_size: 10,
            min_idle: Some(2),
            connection_timeout: Duration::from_secs(30),
        }
    }

    /// Set the maximum number of connections in the pool.
    pub fn with_max_size(mut self, max_size: u32) -> Self {
        self.max_size = max_size;
        self
    }

    /// Set the minimum number of idle connections to maintain.
    pub fn with_min_idle(mut self, min_idle: Option<u32>) -> Self {
        self.min_idle = min_idle;
        self
    }

    /// Set the connection checkout timeout.
    pub fn with_connection_timeout(mut self, timeout: Duration) -> Self {
        self.connection_timeout = timeout;
        self
    }

    /// Get the database URL.
    pub fn database_url(&self) -> &str {
        &self.database_url
    }
}

/// Async connection pool for PostgreSQL via Diesel.
///
/// This wrapper provides a simple interface for obtaining pooled connections
/// and executing database operations. It uses `diesel-async` for native async
/// support without blocking the async runtime.
///
/// # Example
///
/// ```ignore
/// let pool = DbPool::new(config).await?;
/// let mut conn = pool.get().await?;
/// // Use conn for Diesel operations...
/// ```
#[derive(Clone)]
pub struct DbPool {
    inner: Pool<AsyncPgConnection>,
}

impl DbPool {
    /// Create a new connection pool with the given configuration.
    ///
    /// # Errors
    ///
    /// Returns `PoolError::Build` if the pool cannot be constructed (e.g.,
    /// invalid database URL or connection failure).
    pub async fn new(config: PoolConfig) -> Result<Self, PoolError> {
        let manager = AsyncDieselConnectionManager::<AsyncPgConnection>::new(&config.database_url);

        let pool = Pool::builder()
            .max_size(config.max_size)
            .min_idle(config.min_idle)
            .connection_timeout(config.connection_timeout)
            .build(manager)
            .await
            .map_err(|err| PoolError::build(err.to_string()))?;

        Ok(Self { inner: pool })
    }

    /// Get a connection from the pool.
    ///
    /// # Errors
    ///
    /// Returns `PoolError::Checkout` if a connection cannot be obtained within
    /// the configured timeout.
    pub async fn get(
        &self,
    ) -> Result<PooledConnection<'_, AsyncPgConnection>, PoolError> {
        self.inner
            .get()
            .await
            .map_err(|err| PoolError::checkout(err.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    fn pool_config_default_values() {
        let config = PoolConfig::new("postgres://localhost/test");

        assert_eq!(config.database_url(), "postgres://localhost/test");
        assert_eq!(config.max_size, 10);
        assert_eq!(config.min_idle, Some(2));
        assert_eq!(config.connection_timeout, Duration::from_secs(30));
    }

    #[rstest]
    fn pool_config_builder_pattern() {
        let config = PoolConfig::new("postgres://localhost/test")
            .with_max_size(20)
            .with_min_idle(Some(5))
            .with_connection_timeout(Duration::from_secs(60));

        assert_eq!(config.max_size, 20);
        assert_eq!(config.min_idle, Some(5));
        assert_eq!(config.connection_timeout, Duration::from_secs(60));
    }

    #[rstest]
    fn pool_error_display() {
        let checkout_err = PoolError::checkout("connection refused");
        let build_err = PoolError::build("invalid URL");

        assert!(checkout_err.to_string().contains("connection refused"));
        assert!(build_err.to_string().contains("invalid URL"));
    }
}
