//! Environment-driven configuration for idempotency behaviour.

use std::time::Duration;

/// Environment variable name for idempotency TTL configuration.
pub const IDEMPOTENCY_TTL_HOURS_ENV: &str = "IDEMPOTENCY_TTL_HOURS";

/// Environment abstraction for idempotency configuration lookups.
///
/// This trait allows testing with mock environments without unsafe env var
/// mutations.
pub trait IdempotencyEnv {
    /// Fetch a string value by name.
    fn string(&self, name: &str) -> Option<String>;
}

/// Environment access backed by the real process environment.
#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultIdempotencyEnv;

impl DefaultIdempotencyEnv {
    /// Create a new environment reader.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl IdempotencyEnv for DefaultIdempotencyEnv {
    fn string(&self, name: &str) -> Option<String> {
        std::env::var(name).ok()
    }
}

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
#[derive(Debug, Clone)]
pub struct IdempotencyConfig {
    ttl: Duration,
}

impl IdempotencyConfig {
    /// Default TTL in hours.
    const DEFAULT_TTL_HOURS: u64 = 24;

    /// Minimum allowed TTL in hours.
    ///
    /// Prevents pathologically short TTLs that would cause records to expire
    /// before retries can complete.
    const MIN_TTL_HOURS: u64 = 1;

    /// Maximum allowed TTL in hours (10 years).
    ///
    /// Prevents pathologically long TTLs that could cause database bloat or
    /// overflow issues.
    const MAX_TTL_HOURS: u64 = 24 * 365 * 10;

    /// Load configuration from the real process environment.
    ///
    /// Reads `IDEMPOTENCY_TTL_HOURS` (default: 24). Values are clamped to
    /// the range [1, 87600] (1 hour to 10 years) to prevent pathological
    /// configurations.
    ///
    /// # Example
    ///
    /// ```
    /// # use backend::domain::idempotency::IdempotencyConfig;
    /// # use std::time::Duration;
    /// // Without env var set, uses default of 24 hours
    /// let config = IdempotencyConfig::from_env();
    /// assert_eq!(config.ttl(), Duration::from_secs(24 * 3600));
    /// ```
    pub fn from_env() -> Self {
        Self::from_env_with(&DefaultIdempotencyEnv)
    }

    /// Load configuration from a custom environment source.
    ///
    /// Useful for testing without unsafe env var mutations.
    pub fn from_env_with(env: &impl IdempotencyEnv) -> Self {
        let hours = env
            .string(IDEMPOTENCY_TTL_HOURS_ENV)
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(Self::DEFAULT_TTL_HOURS)
            .clamp(Self::MIN_TTL_HOURS, Self::MAX_TTL_HOURS);
        Self {
            ttl: Duration::from_secs(hours.saturating_mul(3600)),
        }
    }

    /// Create with explicit TTL (for testing).
    pub fn with_ttl(ttl: Duration) -> Self {
        Self { ttl }
    }

    /// Returns the configured TTL.
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
