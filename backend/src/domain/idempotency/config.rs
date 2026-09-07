//! Pure configuration value object for idempotency behaviour.
//!
//! The domain owns the TTL policy — its default and its permitted range — but
//! not where the value comes from. Environment loading lives in the
//! `crate::config::idempotency` adapter, which constructs this type from an
//! injected reader (#416, #464).

use std::time::Duration;

/// Configuration for idempotency behaviour.
///
/// Controls the time-to-live (TTL) for idempotency records. Records older than
/// the TTL are eligible for cleanup.
///
/// # Example
///
/// ```
/// # use backend::domain::idempotency::IdempotencyConfig;
/// # use std::time::Duration;
/// let config = IdempotencyConfig::default();
/// assert_eq!(config.ttl(), Duration::from_secs(24 * 3600));
///
/// let custom = IdempotencyConfig::with_ttl(Duration::from_secs(12 * 3600));
/// assert_eq!(custom.ttl(), Duration::from_secs(12 * 3600));
/// ```
#[derive(Debug, Clone, Copy)]
pub struct IdempotencyConfig {
    ttl: Duration,
}

impl IdempotencyConfig {
    /// Default TTL in hours.
    pub const DEFAULT_TTL_HOURS: u64 = 24;

    /// Minimum allowed TTL in hours.
    ///
    /// Prevents pathologically short TTLs that would cause records to expire
    /// before retries can complete.
    pub const MIN_TTL_HOURS: u64 = 1;

    /// Maximum allowed TTL in hours (10 years).
    ///
    /// Prevents pathologically long TTLs that could cause database bloat or
    /// overflow issues.
    pub const MAX_TTL_HOURS: u64 = 24 * 365 * 10;

    /// Build a configuration from an optional TTL in hours.
    ///
    /// `None` selects [`Self::DEFAULT_TTL_HOURS`]; any supplied value is
    /// clamped to `[MIN_TTL_HOURS, MAX_TTL_HOURS]`. The caller decides where
    /// the hours came from, keeping this type free of I/O.
    ///
    /// # Example
    ///
    /// ```
    /// # use backend::domain::idempotency::IdempotencyConfig;
    /// # use std::time::Duration;
    /// let config = IdempotencyConfig::from_ttl_hours(Some(12));
    /// assert_eq!(config.ttl(), Duration::from_secs(12 * 3600));
    ///
    /// let clamped = IdempotencyConfig::from_ttl_hours(Some(0));
    /// assert_eq!(clamped.ttl(), Duration::from_secs(3600));
    /// ```
    #[must_use]
    pub fn from_ttl_hours(hours: Option<u64>) -> Self {
        let hours = hours
            .unwrap_or(Self::DEFAULT_TTL_HOURS)
            .clamp(Self::MIN_TTL_HOURS, Self::MAX_TTL_HOURS);
        Self {
            ttl: Duration::from_secs(hours.saturating_mul(3600)),
        }
    }

    /// Create with explicit TTL (for testing).
    ///
    /// # Example
    ///
    /// ```
    /// # use backend::domain::idempotency::IdempotencyConfig;
    /// # use std::time::Duration;
    /// let config = IdempotencyConfig::with_ttl(Duration::from_secs(3600));
    /// assert_eq!(config.ttl(), Duration::from_secs(3600));
    /// ```
    #[must_use]
    pub fn with_ttl(ttl: Duration) -> Self {
        Self { ttl }
    }

    /// Returns the configured TTL.
    ///
    /// # Example
    ///
    /// ```
    /// # use backend::domain::idempotency::IdempotencyConfig;
    /// # use std::time::Duration;
    /// let config = IdempotencyConfig::with_ttl(Duration::from_secs(7200));
    /// assert_eq!(config.ttl(), Duration::from_secs(7200));
    /// ```
    pub fn ttl(&self) -> Duration {
        self.ttl
    }
}

impl Default for IdempotencyConfig {
    fn default() -> Self {
        Self {
            ttl: Duration::from_secs(Self::DEFAULT_TTL_HOURS * 3600),
        }
    }
}
