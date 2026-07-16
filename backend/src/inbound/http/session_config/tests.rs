//! Unit tests for session configuration parsing.

use super::test_utils::TempKeyFile;
use super::*;
use mockable::{Env as MockableEnv, MockEnv};
use rstest::rstest;
use std::collections::HashMap;
use std::error::Error as StdError;

type TestResult<T = ()> = Result<T, Box<dyn StdError>>;

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
    key_len: Option<usize>,
}

impl TestEnvBuilder {
    fn new() -> Self {
        Self {
            vars: HashMap::new(),
            key_len: None,
        }
    }

    fn with_valid_key(self) -> Self {
        self.with_key_len(SESSION_KEY_MIN_LEN)
    }

    fn with_key_len(mut self, len: usize) -> Self {
        self.key_len = Some(len);
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

    fn build(self) -> std::io::Result<TestEnv> {
        let key_file = match self.key_len {
            Some(len) => Some(TempKeyFile::new(len)?),
            None => None,
        };
        let mut vars = self.vars;
        if let Some(file) = key_file.as_ref() {
            vars.insert(KEY_FILE_ENV.to_string(), file.path_str());
        }
        let env = build_mock_env(vars);
        Ok(TestEnv {
            inner: env,
            _key_file: key_file,
        })
    }
}

fn is_missing_cookie_secure(err: &SessionConfigError) -> bool {
    matches!(
        err,
        SessionConfigError::MissingEnv {
            name: COOKIE_SECURE_ENV
        }
    )
}

fn is_missing_same_site(err: &SessionConfigError) -> bool {
    matches!(err, SessionConfigError::MissingEnv { name: SAMESITE_ENV })
}

fn is_missing_allow_ephemeral(err: &SessionConfigError) -> bool {
    matches!(
        err,
        SessionConfigError::MissingEnv {
            name: ALLOW_EPHEMERAL_ENV
        }
    )
}

fn is_ephemeral_not_allowed(err: &SessionConfigError) -> bool {
    matches!(err, SessionConfigError::EphemeralNotAllowed)
}

fn is_insecure_same_site_none(err: &SessionConfigError) -> bool {
    matches!(err, SessionConfigError::InsecureSameSiteNone)
}

fn is_key_read_error(err: &SessionConfigError) -> bool {
    matches!(err, SessionConfigError::KeyRead { .. })
}

fn is_key_too_short(err: &SessionConfigError) -> bool {
    matches!(err, SessionConfigError::KeyTooShort { .. })
}

#[rstest]
#[case::missing_cookie_secure(
    TestEnvBuilder::new(),
    is_missing_cookie_secure,
    "expected missing cookie secure to fail"
)]
#[case::missing_same_site(
    TestEnvBuilder::new()
        .with_valid_key()
        .with_cookie_secure("1")
        .with_allow_ephemeral("0"),
    is_missing_same_site,
    "expected missing SameSite to fail",
)]
#[case::missing_allow_ephemeral(
    TestEnvBuilder::new()
        .with_valid_key()
        .with_cookie_secure("1")
        .with_same_site("Strict"),
    is_missing_allow_ephemeral,
    "expected missing allow ephemeral to fail",
)]
fn release_missing_env_vars_are_rejected<F>(
    #[case] builder: TestEnvBuilder,
    #[case] matcher: F,
    #[case] description: &str,
) -> TestResult
where
    F: FnOnce(&SessionConfigError) -> bool,
{
    let env = builder.build()?;
    let err = expect_error(
        session_settings_from_env(&env, BuildMode::Release),
        description,
    );
    assert!(matcher(&err), "{description}");
    Ok(())
}

#[rstest]
#[case("maybe")]
#[case("")]
fn release_invalid_cookie_secure_is_rejected(#[case] value: &str) -> TestResult {
    let env = TestEnvBuilder::new()
        .with_valid_key()
        .with_release_defaults()
        .with_cookie_secure(value)
        .build()?;

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
    Ok(())
}

#[rstest]
#[case::ephemeral_enabled(
    TestEnvBuilder::new()
        .with_valid_key()
        .with_release_defaults()
        .with_allow_ephemeral("1"),
    is_ephemeral_not_allowed,
    "expected ephemeral to be rejected in release",
)]
#[case::insecure_same_site_none(
    TestEnvBuilder::new()
        .with_valid_key()
        .with_cookie_secure("0")
        .with_same_site("None")
        .with_allow_ephemeral("0"),
    is_insecure_same_site_none,
    "expected insecure SameSite=None to fail",
)]
fn release_invalid_configurations_are_rejected<F>(
    #[case] builder: TestEnvBuilder,
    #[case] matcher: F,
    #[case] description: &str,
) -> TestResult
where
    F: FnOnce(&SessionConfigError) -> bool,
{
    let env = builder.build()?;
    let err = expect_error(
        session_settings_from_env(&env, BuildMode::Release),
        description,
    );
    assert!(matcher(&err), "{description}");
    Ok(())
}

#[rstest]
#[case::missing_key_file(
    TestEnvBuilder::new()
        .with_cookie_secure("1")
        .with_same_site("Strict")
        .with_allow_ephemeral("0"),
    is_key_read_error,
    "expected missing key file to fail",
)]
#[case::short_key(
    TestEnvBuilder::new()
        .with_key_len(32)
        .with_release_defaults(),
    is_key_too_short,
    "expected short key to fail",
)]
fn release_key_errors_are_rejected<F>(
    #[case] builder: TestEnvBuilder,
    #[case] matcher: F,
    #[case] description: &str,
) -> TestResult
where
    F: FnOnce(&SessionConfigError) -> bool,
{
    let env = builder.build()?;
    let err = expect_error(
        session_settings_from_env(&env, BuildMode::Release),
        description,
    );
    assert!(matcher(&err), "{description}");
    Ok(())
}

#[rstest]
fn release_valid_settings_succeed() -> TestResult {
    let env = TestEnvBuilder::new()
        .with_valid_key()
        .with_release_defaults()
        .build()?;

    let settings = session_settings_from_env(&env, BuildMode::Release)?;
    assert!(settings.cookie_secure);
    assert_eq!(settings.same_site, SameSite::Strict);
    Ok(())
}

#[rstest]
fn debug_defaults_allow_ephemeral_key() -> TestResult {
    let env = TestEnvBuilder::new().build()?;
    let settings = session_settings_from_env(&env, BuildMode::Debug)?;
    assert!(settings.cookie_secure);
    assert_eq!(settings.same_site, SameSite::Lax);
    Ok(())
}

#[rstest]
fn debug_invalid_same_site_falls_back_to_default() -> TestResult {
    let env = TestEnvBuilder::new()
        .with_valid_key()
        .with_cookie_secure("1")
        .with_same_site("unexpected")
        .with_allow_ephemeral("0")
        .build()?;

    let settings = session_settings_from_env(&env, BuildMode::Debug)?;
    assert_eq!(settings.same_site, SameSite::Lax);
    Ok(())
}

#[rstest]
fn debug_explicit_overrides_are_applied() -> TestResult {
    let env = TestEnvBuilder::new()
        .with_valid_key()
        .with_cookie_secure("0")
        .with_same_site("Strict")
        .with_allow_ephemeral("0")
        .build()?;

    let settings = session_settings_from_env(&env, BuildMode::Debug)?;
    assert!(!settings.cookie_secure);
    assert_eq!(settings.same_site, SameSite::Strict);
    Ok(())
}
