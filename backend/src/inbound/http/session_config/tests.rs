//! Unit tests for session configuration parsing.

use super::*;
use mockable::{Env as MockableEnv, MockEnv};
use rstest::rstest;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug)]
struct TempKeyFile {
    path: PathBuf,
}

impl TempKeyFile {
    fn new(len: usize) -> std::io::Result<Self> {
        let path = std::env::temp_dir().join(format!("session-key-{}", Uuid::new_v4()));
        std::fs::write(&path, vec![b'a'; len])?;
        Ok(Self { path })
    }

    fn path_str(&self) -> &str {
        self.path
            .to_str()
            .expect("temporary path should be valid UTF-8")
    }
}

impl Drop for TempKeyFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

struct TestEnv {
    inner: MockEnv,
    _key_file: Option<TempKeyFile>,
}

impl SessionEnv for TestEnv {
    fn string(&self, name: &str) -> Option<String> {
        MockableEnv::string(&self.inner, name)
    }
}

fn build_mock_env(vars: HashMap<String, String>) -> MockEnv {
    let mut env = MockEnv::new();
    env.expect_string()
        .times(0..)
        .returning(move |key| vars.get(key).cloned());
    env
}

fn expect_error(
    result: Result<SessionSettings, SessionConfigError>,
    label: &str,
) -> SessionConfigError {
    match result {
        Ok(_) => panic!("{label}"),
        Err(error) => error,
    }
}

struct TestEnvBuilder {
    vars: HashMap<String, String>,
    _key_file: Option<TempKeyFile>,
}

impl TestEnvBuilder {
    fn new() -> Self {
        Self {
            vars: HashMap::new(),
            _key_file: None,
        }
    }

    fn with_valid_key(self) -> Self {
        self.with_key_len(SESSION_KEY_MIN_LEN)
    }

    fn with_key_len(mut self, len: usize) -> Self {
        let key_file = TempKeyFile::new(len).expect("key file creation should succeed");
        self.vars
            .insert(KEY_FILE_ENV.to_string(), key_file.path_str().to_string());
        self._key_file = Some(key_file);
        self
    }

    fn with_cookie_secure(mut self, value: &str) -> Self {
        self.vars
            .insert(COOKIE_SECURE_ENV.to_string(), value.to_string());
        self
    }

    fn with_same_site(mut self, value: &str) -> Self {
        self.vars
            .insert(SAMESITE_ENV.to_string(), value.to_string());
        self
    }

    fn with_allow_ephemeral(mut self, value: &str) -> Self {
        self.vars
            .insert(ALLOW_EPHEMERAL_ENV.to_string(), value.to_string());
        self
    }

    fn with_release_defaults(self) -> Self {
        self.with_cookie_secure("1")
            .with_same_site("Strict")
            .with_allow_ephemeral("0")
    }

    fn build(self) -> TestEnv {
        let env = build_mock_env(self.vars);
        TestEnv {
            inner: env,
            _key_file: self._key_file,
        }
    }
}

#[rstest]
fn release_missing_cookie_secure_is_rejected() {
    let env = TestEnvBuilder::new().build();
    let err = expect_error(
        session_settings_from_env(&env, BuildMode::Release),
        "expected missing cookie secure to fail",
    );
    assert!(matches!(
        err,
        SessionConfigError::MissingEnv {
            name: COOKIE_SECURE_ENV
        }
    ));
}

#[rstest]
#[case("maybe")]
#[case("")]
fn release_invalid_cookie_secure_is_rejected(#[case] value: &str) {
    let env = TestEnvBuilder::new()
        .with_valid_key()
        .with_release_defaults()
        .with_cookie_secure(value)
        .build();

    let err = expect_error(
        session_settings_from_env(&env, BuildMode::Release),
        "expected invalid cookie secure to fail",
    );
    assert!(matches!(
        err,
        SessionConfigError::InvalidEnv {
            name: COOKIE_SECURE_ENV,
            ..
        }
    ));
}

#[rstest]
fn release_missing_same_site_is_rejected() {
    let env = TestEnvBuilder::new()
        .with_valid_key()
        .with_cookie_secure("1")
        .with_allow_ephemeral("0")
        .build();

    let err = expect_error(
        session_settings_from_env(&env, BuildMode::Release),
        "expected missing SameSite to fail",
    );
    assert!(matches!(
        err,
        SessionConfigError::MissingEnv { name: SAMESITE_ENV }
    ));
}

#[rstest]
fn release_missing_allow_ephemeral_is_rejected() {
    let env = TestEnvBuilder::new()
        .with_valid_key()
        .with_cookie_secure("1")
        .with_same_site("Strict")
        .build();

    let err = expect_error(
        session_settings_from_env(&env, BuildMode::Release),
        "expected missing allow ephemeral to fail",
    );
    assert!(matches!(
        err,
        SessionConfigError::MissingEnv {
            name: ALLOW_EPHEMERAL_ENV
        }
    ));
}

#[rstest]
fn release_ephemeral_enabled_is_rejected() {
    let env = TestEnvBuilder::new()
        .with_valid_key()
        .with_release_defaults()
        .with_allow_ephemeral("1")
        .build();

    let err = expect_error(
        session_settings_from_env(&env, BuildMode::Release),
        "expected ephemeral to be rejected in release",
    );
    assert!(matches!(err, SessionConfigError::EphemeralNotAllowed));
}

#[rstest]
fn release_missing_key_file_is_rejected() {
    let env = TestEnvBuilder::new()
        .with_cookie_secure("1")
        .with_same_site("Strict")
        .with_allow_ephemeral("0")
        .build();

    let err = expect_error(
        session_settings_from_env(&env, BuildMode::Release),
        "expected missing key file to fail",
    );
    assert!(matches!(err, SessionConfigError::KeyRead { .. }));
}

#[rstest]
fn release_short_key_is_rejected() {
    let env = TestEnvBuilder::new()
        .with_key_len(32)
        .with_release_defaults()
        .build();

    let err = expect_error(
        session_settings_from_env(&env, BuildMode::Release),
        "expected short key to fail",
    );
    assert!(matches!(err, SessionConfigError::KeyTooShort { .. }));
}

#[rstest]
fn release_insecure_none_same_site_is_rejected() {
    let env = TestEnvBuilder::new()
        .with_valid_key()
        .with_cookie_secure("0")
        .with_same_site("None")
        .with_allow_ephemeral("0")
        .build();

    let err = expect_error(
        session_settings_from_env(&env, BuildMode::Release),
        "expected insecure SameSite=None to fail",
    );
    assert!(matches!(err, SessionConfigError::InsecureSameSiteNone));
}

#[rstest]
fn release_valid_settings_succeed() {
    let env = TestEnvBuilder::new()
        .with_valid_key()
        .with_release_defaults()
        .build();

    let settings =
        session_settings_from_env(&env, BuildMode::Release).expect("expected valid settings");
    assert!(settings.cookie_secure);
    assert_eq!(settings.same_site, SameSite::Strict);
}

#[rstest]
fn debug_defaults_allow_ephemeral_key() {
    let env = TestEnvBuilder::new().build();
    let settings =
        session_settings_from_env(&env, BuildMode::Debug).expect("debug defaults should succeed");
    assert!(settings.cookie_secure);
    assert_eq!(settings.same_site, SameSite::Lax);
}

#[rstest]
fn debug_invalid_same_site_falls_back_to_default() {
    let env = TestEnvBuilder::new()
        .with_valid_key()
        .with_cookie_secure("1")
        .with_same_site("unexpected")
        .with_allow_ephemeral("0")
        .build();

    let settings = session_settings_from_env(&env, BuildMode::Debug)
        .expect("debug should fall back to defaults");
    assert_eq!(settings.same_site, SameSite::Lax);
}

#[rstest]
fn debug_explicit_overrides_are_applied() {
    let env = TestEnvBuilder::new()
        .with_valid_key()
        .with_cookie_secure("0")
        .with_same_site("Strict")
        .with_allow_ephemeral("0")
        .build();

    let settings = session_settings_from_env(&env, BuildMode::Debug)
        .expect("debug should accept explicit overrides");
    assert!(!settings.cookie_secure);
    assert_eq!(settings.same_site, SameSite::Strict);
}
