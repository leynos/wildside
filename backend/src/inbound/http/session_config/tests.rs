//! Unit tests for session configuration parsing.

use super::*;
use mockable::MockEnv;
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

fn mock_env(vars: HashMap<String, String>) -> MockEnv {
    let mut env = MockEnv::new();
    env.expect_string()
        .times(0..)
        .returning(move |key| vars.get(key).cloned());
    env
}

fn release_defaults(key_path: &str) -> HashMap<String, String> {
    let mut vars = HashMap::new();
    vars.insert(KEY_FILE_ENV.to_string(), key_path.to_string());
    vars.insert(COOKIE_SECURE_ENV.to_string(), "1".to_string());
    vars.insert(SAMESITE_ENV.to_string(), "Strict".to_string());
    vars.insert(ALLOW_EPHEMERAL_ENV.to_string(), "0".to_string());
    vars
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

#[rstest]
fn release_missing_cookie_secure_is_rejected() {
    let env = mock_env(HashMap::new());
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
    let key_file = TempKeyFile::new(SESSION_KEY_MIN_LEN).expect("key file creation should succeed");
    let mut vars = release_defaults(key_file.path_str());
    vars.insert(COOKIE_SECURE_ENV.to_string(), value.to_string());
    let env = mock_env(vars);

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
    let key_file = TempKeyFile::new(SESSION_KEY_MIN_LEN).expect("key file creation should succeed");
    let mut vars = HashMap::new();
    vars.insert(KEY_FILE_ENV.to_string(), key_file.path_str().to_string());
    vars.insert(COOKIE_SECURE_ENV.to_string(), "1".to_string());
    vars.insert(ALLOW_EPHEMERAL_ENV.to_string(), "0".to_string());
    let env = mock_env(vars);

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
    let key_file = TempKeyFile::new(SESSION_KEY_MIN_LEN).expect("key file creation should succeed");
    let mut vars = HashMap::new();
    vars.insert(KEY_FILE_ENV.to_string(), key_file.path_str().to_string());
    vars.insert(COOKIE_SECURE_ENV.to_string(), "1".to_string());
    vars.insert(SAMESITE_ENV.to_string(), "Strict".to_string());
    let env = mock_env(vars);

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
    let key_file = TempKeyFile::new(SESSION_KEY_MIN_LEN).expect("key file creation should succeed");
    let mut vars = release_defaults(key_file.path_str());
    vars.insert(ALLOW_EPHEMERAL_ENV.to_string(), "1".to_string());
    let env = mock_env(vars);

    let err = expect_error(
        session_settings_from_env(&env, BuildMode::Release),
        "expected ephemeral to be rejected in release",
    );
    assert!(matches!(err, SessionConfigError::EphemeralNotAllowed));
}

#[rstest]
fn release_missing_key_file_is_rejected() {
    let mut vars = HashMap::new();
    vars.insert(COOKIE_SECURE_ENV.to_string(), "1".to_string());
    vars.insert(SAMESITE_ENV.to_string(), "Strict".to_string());
    vars.insert(ALLOW_EPHEMERAL_ENV.to_string(), "0".to_string());
    let env = mock_env(vars);

    let err = expect_error(
        session_settings_from_env(&env, BuildMode::Release),
        "expected missing key file to fail",
    );
    assert!(matches!(err, SessionConfigError::KeyRead { .. }));
}

#[rstest]
fn release_short_key_is_rejected() {
    let key_file = TempKeyFile::new(32).expect("key file creation should succeed");
    let env = mock_env(release_defaults(key_file.path_str()));

    let err = expect_error(
        session_settings_from_env(&env, BuildMode::Release),
        "expected short key to fail",
    );
    assert!(matches!(err, SessionConfigError::KeyTooShort { .. }));
}

#[rstest]
fn release_insecure_none_same_site_is_rejected() {
    let key_file = TempKeyFile::new(SESSION_KEY_MIN_LEN).expect("key file creation should succeed");
    let mut vars = HashMap::new();
    vars.insert(KEY_FILE_ENV.to_string(), key_file.path_str().to_string());
    vars.insert(COOKIE_SECURE_ENV.to_string(), "0".to_string());
    vars.insert(SAMESITE_ENV.to_string(), "None".to_string());
    vars.insert(ALLOW_EPHEMERAL_ENV.to_string(), "0".to_string());
    let env = mock_env(vars);

    let err = expect_error(
        session_settings_from_env(&env, BuildMode::Release),
        "expected insecure SameSite=None to fail",
    );
    assert!(matches!(err, SessionConfigError::InsecureSameSiteNone));
}

#[rstest]
fn release_valid_settings_succeed() {
    let key_file = TempKeyFile::new(SESSION_KEY_MIN_LEN).expect("key file creation should succeed");
    let env = mock_env(release_defaults(key_file.path_str()));

    let settings =
        session_settings_from_env(&env, BuildMode::Release).expect("expected valid settings");
    assert!(settings.cookie_secure);
    assert_eq!(settings.same_site, SameSite::Strict);
}

#[rstest]
fn debug_defaults_allow_ephemeral_key() {
    let env = mock_env(HashMap::new());
    let settings =
        session_settings_from_env(&env, BuildMode::Debug).expect("debug defaults should succeed");
    assert!(settings.cookie_secure);
    assert_eq!(settings.same_site, SameSite::Lax);
}

#[rstest]
fn debug_invalid_same_site_falls_back_to_default() {
    let key_file = TempKeyFile::new(SESSION_KEY_MIN_LEN).expect("key file creation should succeed");
    let mut vars = HashMap::new();
    vars.insert(KEY_FILE_ENV.to_string(), key_file.path_str().to_string());
    vars.insert(COOKIE_SECURE_ENV.to_string(), "1".to_string());
    vars.insert(SAMESITE_ENV.to_string(), "unexpected".to_string());
    vars.insert(ALLOW_EPHEMERAL_ENV.to_string(), "0".to_string());
    let env = mock_env(vars);

    let settings = session_settings_from_env(&env, BuildMode::Debug)
        .expect("debug should fall back to defaults");
    assert_eq!(settings.same_site, SameSite::Lax);
}
