//! Session configuration parsing and validation.
//!
//! This module centralises the environment-driven session settings so they are
//! validated consistently and can be tested in isolation.

use actix_web::cookie::{Key, SameSite};
use mockable::Env;
use std::path::PathBuf;
use tracing::warn;
use zeroize::Zeroize;

const SESSION_KEY_DEFAULT_PATH: &str = "/var/run/secrets/session_key";
const SESSION_KEY_MIN_LEN: usize = 64;
const COOKIE_SECURE_ENV: &str = "SESSION_COOKIE_SECURE";
const SAMESITE_ENV: &str = "SESSION_SAMESITE";
const ALLOW_EPHEMERAL_ENV: &str = "SESSION_ALLOW_EPHEMERAL";
const KEY_FILE_ENV: &str = "SESSION_KEY_FILE";
const BOOL_EXPECTED: &str = "1|0|true|false|yes|no|y|n";
const SAMESITE_EXPECTED: &str = "Strict|Lax|None";

/// Build mode for session configuration validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BuildMode {
    /// Debug builds tolerate defaults and emit warnings for missing toggles.
    Debug,
    /// Release builds require explicit, valid session toggles.
    Release,
}

impl BuildMode {
    /// Determine the build mode from `cfg!(debug_assertions)`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use backend::inbound::http::session_config::BuildMode;
    ///
    /// let mode = BuildMode::from_debug_assertions();
    /// if cfg!(debug_assertions) {
    ///     assert_eq!(mode, BuildMode::Debug);
    /// } else {
    ///     assert_eq!(mode, BuildMode::Release);
    /// }
    /// ```
    #[must_use]
    pub fn from_debug_assertions() -> Self {
        if cfg!(debug_assertions) {
            Self::Debug
        } else {
            Self::Release
        }
    }

    fn is_debug(self) -> bool {
        matches!(self, Self::Debug)
    }
}

/// Session settings derived from configuration toggles.
pub struct SessionSettings {
    /// Signing key for cookie sessions.
    pub key: Key,
    /// Whether session cookies are marked `Secure`.
    pub cookie_secure: bool,
    /// Configured `SameSite` policy for session cookies.
    pub same_site: SameSite,
}

/// Errors raised while validating session configuration.
#[derive(thiserror::Error, Debug)]
pub enum SessionConfigError {
    /// A required environment variable is missing.
    #[error("missing required environment variable: {name}")]
    MissingEnv { name: &'static str },
    /// A variable is present but contains an invalid value.
    #[error("invalid value for {name}='{value}'; expected {expected}")]
    InvalidEnv {
        name: &'static str,
        value: String,
        expected: &'static str,
    },
    /// Reading the session key file failed.
    #[error("failed to read session key at {path}: {source}")]
    KeyRead {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// The session key file exists but is too short for release builds.
    #[error("session key at {path} too short: need >= {min_len} bytes, got {length}")]
    KeyTooShort {
        path: PathBuf,
        length: usize,
        min_len: usize,
    },
    /// `SameSite=None` requires a secure cookie setting in release builds.
    #[error("SESSION_SAMESITE=None requires SESSION_COOKIE_SECURE=1")]
    InsecureSameSiteNone,
    /// Release builds must not allow ephemeral session keys.
    #[error("SESSION_ALLOW_EPHEMERAL must be 0 in release builds")]
    EphemeralNotAllowed,
}

/// Build session settings from environment variables and build mode.
///
/// # Examples
///
/// ```rust
/// use backend::inbound::http::session_config::{
///     session_settings_from_env, BuildMode,
/// };
/// use mockable::MockEnv;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let key_path = std::env::temp_dir().join("session_key_example");
/// std::fs::write(&key_path, vec![b'a'; 64])?;
///
/// let key_path = key_path.to_str().expect("valid path").to_string();
/// let mut env = MockEnv::new();
/// env.expect_string()
///     .returning(move |name| match name {
///         "SESSION_KEY_FILE" => Some(key_path.clone()),
///         "SESSION_COOKIE_SECURE" => Some("1".to_string()),
///         "SESSION_SAMESITE" => Some("Strict".to_string()),
///         "SESSION_ALLOW_EPHEMERAL" => Some("0".to_string()),
///         _ => None,
///     });
///
/// let settings = session_settings_from_env(&env, BuildMode::Release)?;
/// assert!(settings.cookie_secure);
///
/// std::fs::remove_file(&key_path)?;
/// # Ok(())
/// # }
/// ```
pub fn session_settings_from_env<E: Env>(
    env: &E,
    mode: BuildMode,
) -> Result<SessionSettings, SessionConfigError> {
    let cookie_secure = cookie_secure_from_env(env, mode)?;
    let same_site = same_site_from_env(env, mode, cookie_secure)?;
    let allow_ephemeral = allow_ephemeral_from_env(env, mode)?;
    let key = session_key_from_env(env, mode, allow_ephemeral)?;

    Ok(SessionSettings {
        key,
        cookie_secure,
        same_site,
    })
}

fn cookie_secure_from_env<E: Env>(env: &E, mode: BuildMode) -> Result<bool, SessionConfigError> {
    match env.string(COOKIE_SECURE_ENV) {
        Some(value) => match parse_bool(&value) {
            Some(flag) => Ok(flag),
            None => {
                if mode.is_debug() {
                    warn!(
                        value = %value,
                        "invalid SESSION_COOKIE_SECURE; defaulting to secure"
                    );
                    Ok(true)
                } else {
                    Err(SessionConfigError::InvalidEnv {
                        name: COOKIE_SECURE_ENV,
                        value,
                        expected: BOOL_EXPECTED,
                    })
                }
            }
        },
        None => {
            if mode.is_debug() {
                warn!("SESSION_COOKIE_SECURE not set; defaulting to secure");
                Ok(true)
            } else {
                Err(SessionConfigError::MissingEnv {
                    name: COOKIE_SECURE_ENV,
                })
            }
        }
    }
}

fn same_site_from_env<E: Env>(
    env: &E,
    mode: BuildMode,
    cookie_secure: bool,
) -> Result<SameSite, SessionConfigError> {
    let default_same_site = if mode.is_debug() {
        SameSite::Lax
    } else {
        SameSite::Strict
    };

    let value = match env.string(SAMESITE_ENV) {
        Some(value) => value,
        None => {
            if mode.is_debug() {
                warn!("SESSION_SAMESITE not set; using default");
                return Ok(default_same_site);
            }
            return Err(SessionConfigError::MissingEnv { name: SAMESITE_ENV });
        }
    };

    let same_site = match value.to_ascii_lowercase().as_str() {
        "lax" => SameSite::Lax,
        "strict" => SameSite::Strict,
        "none" => {
            if !cookie_secure {
                if mode.is_debug() {
                    warn!(
                        "{}",
                        concat!(
                            "SESSION_SAMESITE=None with SESSION_COOKIE_SECURE=0; ",
                            "browsers may reject third-party cookies"
                        )
                    );
                } else {
                    return Err(SessionConfigError::InsecureSameSiteNone);
                }
            }
            SameSite::None
        }
        _ => {
            if mode.is_debug() {
                warn!(value = %value, "invalid SESSION_SAMESITE, using default");
                return Ok(default_same_site);
            }
            return Err(SessionConfigError::InvalidEnv {
                name: SAMESITE_ENV,
                value,
                expected: SAMESITE_EXPECTED,
            });
        }
    };

    Ok(same_site)
}

fn allow_ephemeral_from_env<E: Env>(env: &E, mode: BuildMode) -> Result<bool, SessionConfigError> {
    match env.string(ALLOW_EPHEMERAL_ENV) {
        Some(value) => match parse_bool(&value) {
            Some(true) => {
                if mode.is_debug() {
                    Ok(true)
                } else {
                    Err(SessionConfigError::EphemeralNotAllowed)
                }
            }
            Some(false) => Ok(false),
            None => {
                if mode.is_debug() {
                    warn!(
                        value = %value,
                        "invalid SESSION_ALLOW_EPHEMERAL; defaulting to disabled"
                    );
                    Ok(false)
                } else {
                    Err(SessionConfigError::InvalidEnv {
                        name: ALLOW_EPHEMERAL_ENV,
                        value,
                        expected: BOOL_EXPECTED,
                    })
                }
            }
        },
        None => {
            if mode.is_debug() {
                warn!("SESSION_ALLOW_EPHEMERAL not set; defaulting to disabled");
                Ok(false)
            } else {
                Err(SessionConfigError::MissingEnv {
                    name: ALLOW_EPHEMERAL_ENV,
                })
            }
        }
    }
}

fn session_key_from_env<E: Env>(
    env: &E,
    mode: BuildMode,
    allow_ephemeral: bool,
) -> Result<Key, SessionConfigError> {
    let key_path = env
        .string(KEY_FILE_ENV)
        .unwrap_or_else(|| SESSION_KEY_DEFAULT_PATH.to_string());
    let path = PathBuf::from(key_path);

    match std::fs::read(&path) {
        Ok(mut bytes) => {
            let length = bytes.len();
            if mode == BuildMode::Release && length < SESSION_KEY_MIN_LEN {
                bytes.zeroize();
                return Err(SessionConfigError::KeyTooShort {
                    path,
                    length,
                    min_len: SESSION_KEY_MIN_LEN,
                });
            }
            let key = Key::derive_from(&bytes);
            bytes.zeroize();
            Ok(key)
        }
        Err(error) => {
            if mode.is_debug() || allow_ephemeral {
                warn!(
                    path = %path.display(),
                    error = %error,
                    "using temporary session key (dev only)"
                );
                Ok(Key::generate())
            } else {
                Err(SessionConfigError::KeyRead {
                    path,
                    source: error,
                })
            }
        }
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "y" => Some(true),
        "0" | "false" | "no" | "n" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
