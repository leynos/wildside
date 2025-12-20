//! Behaviour tests for session configuration toggles.
//!
//! These scenarios validate that release builds enforce explicit toggle
//! configuration and reject insecure or missing settings.

use backend::inbound::http::session_config::{
    session_settings_from_env, BuildMode, SessionConfigError, SessionEnv, SessionSettings,
    ALLOW_EPHEMERAL_ENV, COOKIE_SECURE_ENV, KEY_FILE_ENV, SAMESITE_ENV,
};
use mockable::{Env as MockableEnv, MockEnv};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

struct TestEnv {
    inner: MockEnv,
}

impl SessionEnv for TestEnv {
    fn string(&self, name: &str) -> Option<String> {
        MockableEnv::string(&self.inner, name)
    }
}

struct SessionConfigWorld {
    vars: RefCell<HashMap<String, String>>,
    mode: RefCell<BuildMode>,
    outcome: RefCell<Option<Result<SessionSettings, SessionConfigError>>>,
    key_files: RefCell<Vec<TempKeyFile>>,
}

impl SessionConfigWorld {
    fn new() -> Self {
        Self {
            vars: RefCell::new(HashMap::new()),
            mode: RefCell::new(BuildMode::Release),
            outcome: RefCell::new(None),
            key_files: RefCell::new(Vec::new()),
        }
    }

    fn set_mode(&self, mode: BuildMode) {
        *self.mode.borrow_mut() = mode;
    }

    fn set_env_var(&self, name: &str, value: &str) {
        self.vars
            .borrow_mut()
            .insert(name.to_string(), value.to_string());
    }

    fn add_key_file(&self, len: usize) {
        let file = TempKeyFile::new(len).expect("key file creation should succeed");
        self.set_env_var(KEY_FILE_ENV, file.path_str());
        self.key_files.borrow_mut().push(file);
    }

    fn evaluate(&self) {
        let env = mock_env(self.vars.borrow().clone());
        let mode = *self.mode.borrow();
        let result = session_settings_from_env(&env, mode);
        *self.outcome.borrow_mut() = Some(result);
    }

    fn with_settings<F>(&self, f: F)
    where
        F: FnOnce(&SessionSettings),
    {
        let outcome = self.outcome.borrow();
        let settings = outcome
            .as_ref()
            .expect("evaluation result")
            .as_ref()
            .expect("expected settings to succeed");
        f(settings);
    }

    fn with_error<F>(&self, f: F)
    where
        F: FnOnce(&SessionConfigError),
    {
        let outcome = self.outcome.borrow();
        let error = match outcome.as_ref().expect("evaluation result") {
            Ok(_) => panic!("expected settings to fail"),
            Err(error) => error,
        };
        f(error);
    }
}

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

fn mock_env(vars: HashMap<String, String>) -> TestEnv {
    let mut env = MockEnv::new();
    env.expect_string()
        .times(0..)
        .returning(move |key| vars.get(key).cloned());
    TestEnv { inner: env }
}

#[fixture]
fn world() -> SessionConfigWorld {
    SessionConfigWorld::new()
}

#[given("a release build configuration")]
fn a_release_build_configuration(world: &SessionConfigWorld) {
    world.set_mode(BuildMode::Release);
}

#[given("SESSION_COOKIE_SECURE is set to {value}")]
fn session_cookie_secure_is_set(world: &SessionConfigWorld, value: String) {
    world.set_env_var(COOKIE_SECURE_ENV, &value);
}

#[given("SESSION_SAMESITE is set to {value}")]
fn session_same_site_is_set(world: &SessionConfigWorld, value: String) {
    world.set_env_var(SAMESITE_ENV, &value);
}

#[given("SESSION_ALLOW_EPHEMERAL is set to {value}")]
fn session_allow_ephemeral_is_set(world: &SessionConfigWorld, value: String) {
    world.set_env_var(ALLOW_EPHEMERAL_ENV, &value);
}

#[given("a session key file with {len} bytes")]
fn a_session_key_file_with_bytes(world: &SessionConfigWorld, len: usize) {
    world.add_key_file(len);
}

#[when("the session configuration is loaded")]
fn the_session_configuration_is_loaded(world: &SessionConfigWorld) {
    world.evaluate();
}

#[then("the configuration load succeeds")]
fn the_configuration_load_succeeds(world: &SessionConfigWorld) {
    world.with_settings(|_| {});
}

#[then("the cookie secure flag is true")]
fn the_cookie_secure_flag_is_true(world: &SessionConfigWorld) {
    world.with_settings(|settings| {
        assert!(settings.cookie_secure);
    });
}

#[then("the SameSite policy is Strict")]
fn the_same_site_policy_is_strict(world: &SessionConfigWorld) {
    world.with_settings(|settings| {
        assert_eq!(settings.same_site, actix_web::cookie::SameSite::Strict);
    });
}

#[then("the configuration load fails due to missing SESSION_COOKIE_SECURE")]
fn configuration_fails_missing_cookie_secure(world: &SessionConfigWorld) {
    world.with_error(|error| {
        assert!(matches!(
            error,
            SessionConfigError::MissingEnv {
                name: COOKIE_SECURE_ENV
            }
        ));
    });
}

#[then("the configuration load fails because ephemeral keys are not allowed")]
fn configuration_fails_ephemeral_not_allowed(world: &SessionConfigWorld) {
    world.with_error(|error| {
        assert!(matches!(error, SessionConfigError::EphemeralNotAllowed));
    });
}

#[then("the configuration load fails because SameSite=None requires secure cookies")]
fn configuration_fails_insecure_same_site_none(world: &SessionConfigWorld) {
    world.with_error(|error| {
        assert!(matches!(error, SessionConfigError::InsecureSameSiteNone));
    });
}

#[then("the configuration load fails because the key is too short")]
fn configuration_fails_key_too_short(world: &SessionConfigWorld) {
    world.with_error(|error| {
        assert!(matches!(error, SessionConfigError::KeyTooShort { .. }));
    });
}

#[scenario(path = "tests/features/session_config.feature")]
fn session_configuration_scenarios(world: SessionConfigWorld) {
    drop(world);
}
