//! Session configuration parsing and validation.
//!
//! This module centralises the environment-driven session settings so they are
//! validated consistently and can be tested in isolation.

use actix_web::cookie::{Key, SameSite};
use cap_std::{ambient_authority, fs::Dir};
use parsing::{BoolEnvConfig, debug_warn_or_error, parse_bool_env, parse_same_site_value};
use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};
use tracing::warn;
use zeroize::Zeroize;

const SESSION_KEY_DEFAULT_PATH: &str = "/var/run/secrets/session_key";
pub const SESSION_KEY_MIN_LEN: usize = 64;
pub const COOKIE_SECURE_ENV: &str = "SESSION_COOKIE_SECURE";
pub const SAMESITE_ENV: &str = "SESSION_SAMESITE";
pub const ALLOW_EPHEMERAL_ENV: &str = "SESSION_ALLOW_EPHEMERAL";
pub const KEY_FILE_ENV: &str = "SESSION_KEY_FILE";

/// Environment abstraction for session configuration lookups.
pub trait SessionEnv {
    /// Fetch a string value by name.
    fn string(&self, name: &str) -> Option<String>;
}

/// Environment access backed by the real process environment.
#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultEnv;

impl DefaultEnv {
    /// Create a new environment reader.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl SessionEnv for DefaultEnv {
    fn string(&self, name: &str) -> Option<String> {
        std::env::var(name).ok()
    }
}

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
    /// Truncated SHA-256 fingerprint of the signing key for operational
    /// visibility.
    pub fingerprint: String,
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
///     session_settings_from_env, BuildMode, SessionEnv,
/// };
/// use mockable::{Env as MockableEnv, MockEnv};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let key_path = std::env::temp_dir().join("session_key_example");
/// std::fs::write(&key_path, vec![b'a'; 64])?;
///
/// let key_path_string = key_path.to_str().expect("valid path").to_string();
/// struct TestEnv {
///     inner: MockEnv,
/// }
/// impl SessionEnv for TestEnv {
///     fn string(&self, name: &str) -> Option<String> {
///         MockableEnv::string(&self.inner, name)
///     }
/// }
/// let mut inner = MockEnv::new();
/// inner.expect_string()
///     .returning(move |name| match name {
///         "SESSION_KEY_FILE" => Some(key_path_string.clone()),
///         "SESSION_COOKIE_SECURE" => Some("1".to_string()),
///         "SESSION_SAMESITE" => Some("Strict".to_string()),
///         "SESSION_ALLOW_EPHEMERAL" => Some("0".to_string()),
///         _ => None,
///     });
/// let env = TestEnv { inner };
///
/// let settings = session_settings_from_env(&env, BuildMode::Release)?;
/// assert!(settings.cookie_secure);
///
/// std::fs::remove_file(&key_path)?;
/// # Ok(())
/// # }
/// ```
pub fn session_settings_from_env<E: SessionEnv>(
    env: &E,
    mode: BuildMode,
) -> Result<SessionSettings, SessionConfigError> {
    let cookie_secure = cookie_secure_from_env(env, mode)?;
    let same_site = same_site_from_env(env, mode, cookie_secure)?;
    let allow_ephemeral = allow_ephemeral_from_env(env, mode)?;
    let key = session_key_from_env(env, mode, allow_ephemeral)?;
    let fingerprint = fingerprint::key_fingerprint(&key);

    Ok(SessionSettings {
        key,
        cookie_secure,
        same_site,
        fingerprint,
    })
}

fn cookie_secure_from_env<E: SessionEnv>(
    env: &E,
    mode: BuildMode,
) -> Result<bool, SessionConfigError> {
    parse_bool_env(
        env,
        mode,
        BoolEnvConfig::new(COOKIE_SECURE_ENV, true),
        |flag, _| Ok(flag),
    )
}

fn same_site_from_env<E: SessionEnv>(
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
            return debug_warn_or_error(
                mode,
                default_same_site,
                SessionConfigError::MissingEnv { name: SAMESITE_ENV },
                || warn!("SESSION_SAMESITE not set; using default"),
            );
        }
    };

    parse_same_site_value(value, mode, cookie_secure, default_same_site)
}

fn allow_ephemeral_from_env<E: SessionEnv>(
    env: &E,
    mode: BuildMode,
) -> Result<bool, SessionConfigError> {
    parse_bool_env(
        env,
        mode,
        BoolEnvConfig::new(ALLOW_EPHEMERAL_ENV, false),
        |flag, mode| {
            if flag && mode == BuildMode::Release {
                Err(SessionConfigError::EphemeralNotAllowed)
            } else {
                Ok(flag)
            }
        },
    )
}

/// Session key validation policy for runtime behaviour.
#[derive(Clone, Copy)]
struct KeyValidationPolicy {
    mode: BuildMode,
    allow_ephemeral: bool,
}

impl KeyValidationPolicy {
    /// Create a validation policy for the current build mode.
    fn new(mode: BuildMode, allow_ephemeral: bool) -> Self {
        Self {
            mode,
            allow_ephemeral,
        }
    }

    /// Decide whether to allow an ephemeral session key.
    fn should_allow_ephemeral(self) -> bool {
        self.mode.is_debug() || self.allow_ephemeral
    }
}

fn session_key_from_env<E: SessionEnv>(
    env: &E,
    mode: BuildMode,
    allow_ephemeral: bool,
) -> Result<Key, SessionConfigError> {
    let policy = KeyValidationPolicy::new(mode, allow_ephemeral);
    let path = resolve_key_path(env)?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = extract_file_name(&path)?;
    let dir = match Dir::open_ambient_dir(parent, ambient_authority()) {
        Ok(dir) => dir,
        Err(error) => {
            return handle_io_error_with_ephemeral_fallback(error, &path, policy);
        }
    };

    read_and_validate_key(&dir, file_name, &path, policy)
}

/// Resolve the configured key path from the environment or the default.
fn resolve_key_path<E: SessionEnv>(env: &E) -> Result<PathBuf, SessionConfigError> {
    let key_path = env
        .string(KEY_FILE_ENV)
        .unwrap_or_else(|| SESSION_KEY_DEFAULT_PATH.to_string());
    Ok(PathBuf::from(key_path))
}

/// Extract the file name from the path, rejecting paths without one.
fn extract_file_name(path: &Path) -> Result<&OsStr, SessionConfigError> {
    path.file_name().ok_or_else(|| SessionConfigError::KeyRead {
        path: path.to_path_buf(),
        source: io::Error::new(
            io::ErrorKind::InvalidInput,
            "session key path must be a file",
        ),
    })
}

/// Handle I/O errors when accessing the session key, with optional ephemeral fallback.
fn handle_io_error_with_ephemeral_fallback(
    error: io::Error,
    path: &Path,
    policy: KeyValidationPolicy,
) -> Result<Key, SessionConfigError> {
    if policy.should_allow_ephemeral() {
        warn!(
            path = %path.display(),
            error = %error,
            "using temporary session key (dev or allow_ephemeral)"
        );
        return Ok(Key::generate());
    }

    Err(SessionConfigError::KeyRead {
        path: path.to_path_buf(),
        source: error,
    })
}

/// Read the key file and validate the contents for the current build mode.
fn read_and_validate_key(
    dir: &Dir,
    file_name: &OsStr,
    full_path: &Path,
    policy: KeyValidationPolicy,
) -> Result<Key, SessionConfigError> {
    match dir.read(Path::new(file_name)) {
        Ok(bytes) => process_key_bytes(bytes, full_path, policy.mode),
        Err(error) => handle_io_error_with_ephemeral_fallback(error, full_path, policy),
    }
}

/// Validate key bytes, derive the key, and zeroize the buffer.
fn process_key_bytes(
    mut bytes: Vec<u8>,
    path: &Path,
    mode: BuildMode,
) -> Result<Key, SessionConfigError> {
    let length = bytes.len();
    if mode == BuildMode::Release && length < SESSION_KEY_MIN_LEN {
        bytes.zeroize();
        return Err(SessionConfigError::KeyTooShort {
            path: path.to_path_buf(),
            length,
            min_len: SESSION_KEY_MIN_LEN,
        });
    }
    let key = Key::derive_from(&bytes);
    bytes.zeroize();
    Ok(key)
}

pub mod fingerprint;
mod parsing;
pub mod test_utils;

#[cfg(test)]
mod tests;
