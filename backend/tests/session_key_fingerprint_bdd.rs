//! Behaviour tests for session key fingerprinting.
//!
//! These scenarios validate that the fingerprint computation is deterministic,
//! produces distinct values for different keys, and follows the expected format.

use actix_web::cookie::Key;
use backend::inbound::http::session_config::{
    ALLOW_EPHEMERAL_ENV, BuildMode, COOKIE_SECURE_ENV, KEY_FILE_ENV, SAMESITE_ENV,
    SessionConfigError, SessionEnv, SessionSettings, fingerprint::key_fingerprint,
    session_settings_from_env, test_utils::TempKeyFile,
};
use mockable::{Env as MockableEnv, MockEnv};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::cell::RefCell;
use std::collections::HashMap;

struct TestEnv {
    inner: MockEnv,
}

impl SessionEnv for TestEnv {
    fn string(&self, name: &str) -> Option<String> {
        MockableEnv::string(&self.inner, name)
    }
}

struct FingerprintWorld {
    key_a: RefCell<Option<Key>>,
    key_b: RefCell<Option<Key>>,
    fingerprint_a: RefCell<Option<String>>,
    fingerprint_b: RefCell<Option<String>>,
    fingerprint_second: RefCell<Option<String>>,
    vars: RefCell<HashMap<String, String>>,
    mode: RefCell<BuildMode>,
    outcome: RefCell<Option<Result<SessionSettings, SessionConfigError>>>,
    key_files: RefCell<Vec<TempKeyFile>>,
}

impl FingerprintWorld {
    fn new() -> Self {
        Self {
            key_a: RefCell::new(None),
            key_b: RefCell::new(None),
            fingerprint_a: RefCell::new(None),
            fingerprint_b: RefCell::new(None),
            fingerprint_second: RefCell::new(None),
            vars: RefCell::new(HashMap::new()),
            mode: RefCell::new(BuildMode::Release),
            outcome: RefCell::new(None),
            key_files: RefCell::new(Vec::new()),
        }
    }

    fn set_key_a(&self, key: Key) {
        *self.key_a.borrow_mut() = Some(key);
    }

    fn set_key_b(&self, key: Key) {
        *self.key_b.borrow_mut() = Some(key);
    }

    fn compute_fingerprint_a(&self) {
        let key = self.key_a.borrow();
        let key = key.as_ref().expect("key_a should be set");
        *self.fingerprint_a.borrow_mut() = Some(key_fingerprint(key));
    }

    fn compute_fingerprint_a_again(&self) {
        let key = self.key_a.borrow();
        let key = key.as_ref().expect("key_a should be set");
        *self.fingerprint_second.borrow_mut() = Some(key_fingerprint(key));
    }

    fn compute_fingerprint_b(&self) {
        let key = self.key_b.borrow();
        let key = key.as_ref().expect("key_b should be set");
        *self.fingerprint_b.borrow_mut() = Some(key_fingerprint(key));
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
        let key_path = file.path_str();
        self.set_env_var(KEY_FILE_ENV, &key_path);
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
}

fn mock_env(vars: HashMap<String, String>) -> TestEnv {
    let mut env = MockEnv::new();
    env.expect_string()
        .times(0..)
        .returning(move |key| vars.get(key).cloned());
    TestEnv { inner: env }
}

#[fixture]
fn world() -> FingerprintWorld {
    FingerprintWorld::new()
}

// Scenario: Fingerprint is deterministic for the same key

#[given("a session key derived from fixed bytes")]
fn a_session_key_derived_from_fixed_bytes(world: &FingerprintWorld) {
    let key = Key::derive_from(&[b'x'; 64]);
    world.set_key_a(key);
}

#[when("the fingerprint is computed twice")]
fn the_fingerprint_is_computed_twice(world: &FingerprintWorld) {
    world.compute_fingerprint_a();
    world.compute_fingerprint_a_again();
}

#[then("both fingerprints are identical")]
fn both_fingerprints_are_identical(world: &FingerprintWorld) {
    let fp1 = world.fingerprint_a.borrow();
    let fp2 = world.fingerprint_second.borrow();
    assert_eq!(
        fp1.as_ref().expect("fingerprint_a"),
        fp2.as_ref().expect("fingerprint_second"),
        "fingerprints should be identical for the same key"
    );
}

// Scenario: Different keys produce different fingerprints

#[given("a session key derived from bytes {byte_char}")]
fn a_session_key_derived_from_bytes(world: &FingerprintWorld, byte_char: char) {
    let byte = byte_char as u8;
    let key = Key::derive_from(&[byte; 64]);
    if world.key_a.borrow().is_none() {
        world.set_key_a(key);
    } else {
        world.set_key_b(key);
    }
}

#[given("another session key derived from bytes {byte_char}")]
fn another_session_key_derived_from_bytes(world: &FingerprintWorld, byte_char: char) {
    let byte = byte_char as u8;
    let key = Key::derive_from(&[byte; 64]);
    world.set_key_b(key);
}

#[when("fingerprints are computed for both keys")]
fn fingerprints_are_computed_for_both_keys(world: &FingerprintWorld) {
    world.compute_fingerprint_a();
    world.compute_fingerprint_b();
}

#[then("the fingerprints differ")]
fn the_fingerprints_differ(world: &FingerprintWorld) {
    let fp_a = world.fingerprint_a.borrow();
    let fp_b = world.fingerprint_b.borrow();
    assert_ne!(
        fp_a.as_ref().expect("fingerprint_a"),
        fp_b.as_ref().expect("fingerprint_b"),
        "fingerprints should differ for different keys"
    );
}

// Scenario: Fingerprint has correct format

#[given("a randomly generated session key")]
fn a_randomly_generated_session_key(world: &FingerprintWorld) {
    let key = Key::generate();
    world.set_key_a(key);
}

#[when("the fingerprint is computed")]
fn the_fingerprint_is_computed(world: &FingerprintWorld) {
    world.compute_fingerprint_a();
}

#[then("the fingerprint is 16 characters long")]
fn the_fingerprint_is_16_characters_long(world: &FingerprintWorld) {
    let fp = world.fingerprint_a.borrow();
    let fp = fp.as_ref().expect("fingerprint_a");
    assert_eq!(fp.len(), 16, "fingerprint should be 16 characters");
}

#[then("the fingerprint contains only hexadecimal characters")]
fn the_fingerprint_contains_only_hexadecimal_characters(world: &FingerprintWorld) {
    let fp = world.fingerprint_a.borrow();
    let fp = fp.as_ref().expect("fingerprint_a");
    assert!(
        fp.chars().all(|c| c.is_ascii_hexdigit()),
        "fingerprint should contain only hex characters"
    );
}

#[then("the fingerprint is lowercase")]
fn the_fingerprint_is_lowercase(world: &FingerprintWorld) {
    let fp = world.fingerprint_a.borrow();
    let fp = fp.as_ref().expect("fingerprint_a");
    assert_eq!(
        fp,
        &fp.to_lowercase(),
        "fingerprint should be lowercase hex"
    );
}

// Scenario: Session settings include fingerprint (reuse session_config steps)

#[given("a release build configuration")]
fn a_release_build_configuration(world: &FingerprintWorld) {
    world.set_mode(BuildMode::Release);
}

#[given("SESSION_COOKIE_SECURE is set to {value}")]
fn session_cookie_secure_is_set(world: &FingerprintWorld, value: String) {
    world.set_env_var(COOKIE_SECURE_ENV, &value);
}

#[given("SESSION_SAMESITE is set to {value}")]
fn session_same_site_is_set(world: &FingerprintWorld, value: String) {
    world.set_env_var(SAMESITE_ENV, &value);
}

#[given("SESSION_ALLOW_EPHEMERAL is set to {value}")]
fn session_allow_ephemeral_is_set(world: &FingerprintWorld, value: String) {
    world.set_env_var(ALLOW_EPHEMERAL_ENV, &value);
}

#[given("a session key file with {len} bytes")]
fn a_session_key_file_with_bytes(world: &FingerprintWorld, len: usize) {
    world.add_key_file(len);
}

#[when("the session configuration is loaded")]
fn the_session_configuration_is_loaded(world: &FingerprintWorld) {
    world.evaluate();
}

#[then("the configuration load succeeds")]
fn the_configuration_load_succeeds(world: &FingerprintWorld) {
    world.with_settings(|_| {});
}

#[then("the settings include a non-empty fingerprint")]
fn the_settings_include_a_non_empty_fingerprint(world: &FingerprintWorld) {
    world.with_settings(|settings| {
        assert!(
            !settings.fingerprint.is_empty(),
            "fingerprint should not be empty"
        );
        assert_eq!(
            settings.fingerprint.len(),
            16,
            "fingerprint should be 16 characters"
        );
    });
}

#[scenario(path = "tests/features/session_key_fingerprint.feature")]
fn session_key_fingerprint_scenarios(world: FingerprintWorld) {
    drop(world);
}
