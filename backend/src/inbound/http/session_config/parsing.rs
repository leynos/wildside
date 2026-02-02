//! Environment parsing helpers for session configuration.

use actix_web::cookie::SameSite;
use tracing::warn;

use super::{BuildMode, SAMESITE_ENV, SessionConfigError, SessionEnv};

const BOOL_EXPECTED: &str = "1|0|true|false|yes|no|y|n";
const SAMESITE_EXPECTED: &str = "Strict|Lax|None";

/// Configuration for parsing a boolean environment variable.
pub(super) struct BoolEnvConfig {
    name: &'static str,
    default_value: bool,
}

impl BoolEnvConfig {
    pub(super) const fn new(name: &'static str, default_value: bool) -> Self {
        Self {
            name,
            default_value,
        }
    }
}

pub(super) fn parse_bool_env<E: SessionEnv, F>(
    env: &E,
    mode: BuildMode,
    config: BoolEnvConfig,
    value_validator: F,
) -> Result<bool, SessionConfigError>
where
    F: FnOnce(bool, BuildMode) -> Result<bool, SessionConfigError>,
{
    let default_label = if config.default_value {
        "enabled"
    } else {
        "disabled"
    };
    match env.string(config.name) {
        Some(value) => match parse_bool(&value) {
            Some(flag) => value_validator(flag, mode),
            None => {
                let value_clone = value.clone();
                debug_warn_or_error(
                    mode,
                    config.default_value,
                    SessionConfigError::InvalidEnv {
                        name: config.name,
                        value: value_clone,
                        expected: BOOL_EXPECTED,
                    },
                    || {
                        warn!(
                            value = %value,
                            "invalid {}; defaulting to {}",
                            config.name,
                            default_label
                        );
                    },
                )
            }
        },
        None => debug_warn_or_error(
            mode,
            config.default_value,
            SessionConfigError::MissingEnv { name: config.name },
            || warn!("{} not set; defaulting to {}", config.name, default_label),
        ),
    }
}

pub(super) fn debug_warn_or_error<T, F>(
    mode: BuildMode,
    fallback: T,
    error: SessionConfigError,
    warn_fn: F,
) -> Result<T, SessionConfigError>
where
    F: FnOnce(),
{
    if mode.is_debug() {
        warn_fn();
        Ok(fallback)
    } else {
        Err(error)
    }
}

pub(super) fn parse_same_site_value(
    value: String,
    mode: BuildMode,
    cookie_secure: bool,
    default_same_site: SameSite,
) -> Result<SameSite, SessionConfigError> {
    let value_lower = value.to_ascii_lowercase();
    match value_lower.as_str() {
        "lax" => Ok(SameSite::Lax),
        "strict" => Ok(SameSite::Strict),
        "none" => {
            validate_same_site_none(mode, cookie_secure)?;
            Ok(SameSite::None)
        }
        _ => debug_warn_or_error(
            mode,
            default_same_site,
            SessionConfigError::InvalidEnv {
                name: SAMESITE_ENV,
                value: value.clone(),
                expected: SAMESITE_EXPECTED,
            },
            || warn!(value = %value, "invalid SESSION_SAMESITE, using default"),
        ),
    }
}

fn validate_same_site_none(mode: BuildMode, cookie_secure: bool) -> Result<(), SessionConfigError> {
    if cookie_secure {
        return Ok(());
    }

    debug_warn_or_error(mode, (), SessionConfigError::InsecureSameSiteNone, || {
        warn!(
            "{}",
            concat!(
                "SESSION_SAMESITE=None with SESSION_COOKIE_SECURE=0; ",
                "browsers may reject third-party cookies"
            )
        );
    })
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "y" => Some(true),
        "0" | "false" | "no" | "n" => Some(false),
        _ => None,
    }
}
