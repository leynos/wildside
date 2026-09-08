//! Process-environment adapter for idempotency configuration.
//!
//! The domain owns the TTL policy ([`IdempotencyConfig`]); this module owns the
//! variable name, the string parsing, and the single process-backed reader.
//! Application wiring injects the reader, so nothing in the domain layer
//! touches the environment (#416, #464).

use crate::domain::idempotency::IdempotencyConfig;

/// Environment variable name for idempotency TTL configuration.
pub const IDEMPOTENCY_TTL_HOURS_ENV: &str = "IDEMPOTENCY_TTL_HOURS";

/// Environment abstraction for idempotency configuration lookups.
///
/// The trait exists because the TTL boundary is exercised by several tests and
/// is expected to grow further inputs; a stub implementation replaces process
/// mutation entirely.
///
/// # Example
///
/// ```
/// # use backend::config::idempotency::IdempotencyEnv;
/// struct StubEnv;
///
/// impl IdempotencyEnv for StubEnv {
///     fn string(&self, name: &str) -> Option<String> {
///         (name == "IDEMPOTENCY_TTL_HOURS").then(|| "12".to_string())
///     }
/// }
///
/// let env = StubEnv;
/// assert_eq!(env.string("IDEMPOTENCY_TTL_HOURS"), Some("12".to_string()));
/// assert_eq!(env.string("OTHER"), None);
/// ```
pub trait IdempotencyEnv {
    /// Fetch a string value by name.
    fn string(&self, name: &str) -> Option<String>;
}

/// Environment access backed by the real process environment.
///
/// # Example
///
/// ```
/// # use backend::config::idempotency::{DefaultIdempotencyEnv, IdempotencyEnv};
/// let env = DefaultIdempotencyEnv::new();
/// let _value = env.string("IDEMPOTENCY_TTL_HOURS");
/// ```
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
    #[expect(
        clippy::disallowed_methods,
        reason = "process-backed adapter: the single sanctioned read behind \
                  IdempotencyEnv, injected from application composition"
    )]
    fn string(&self, name: &str) -> Option<String> {
        std::env::var(name).ok()
    }
}

/// Load idempotency configuration through an injected environment reader.
///
/// Reads `IDEMPOTENCY_TTL_HOURS`; an absent, non-numeric, or out-of-range value
/// falls back to the domain's default and clamp rules.
///
/// # Example
///
/// ```
/// # use backend::config::idempotency::{IdempotencyEnv, idempotency_config_from_env};
/// # use std::time::Duration;
/// struct StubEnv;
///
/// impl IdempotencyEnv for StubEnv {
///     fn string(&self, name: &str) -> Option<String> {
///         (name == "IDEMPOTENCY_TTL_HOURS").then(|| "12".to_string())
///     }
/// }
///
/// let config = idempotency_config_from_env(&StubEnv);
/// assert_eq!(config.ttl(), Duration::from_secs(12 * 3600));
/// ```
#[must_use]
pub fn idempotency_config_from_env(env: &impl IdempotencyEnv) -> IdempotencyConfig {
    let hours = env
        .string(IDEMPOTENCY_TTL_HOURS_ENV)
        .and_then(|value| value.parse::<u64>().ok());
    IdempotencyConfig::from_ttl_hours(hours)
}

#[cfg(test)]
mod tests;
