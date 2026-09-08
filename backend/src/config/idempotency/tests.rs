//! Unit tests for the idempotency configuration adapter.

use std::collections::HashMap;
use std::time::Duration;

use mockable::{Env as MockableEnv, MockEnv};
use rstest::rstest;

use super::{IdempotencyEnv, idempotency_config_from_env};

/// Stub environment backed by `mockable`'s recorded variable map.
struct TestEnv {
    inner: MockEnv,
}

impl IdempotencyEnv for TestEnv {
    fn string(&self, name: &str) -> Option<String> {
        MockableEnv::string(&self.inner, name)
    }
}

/// Build a stub environment exposing the given variables and nothing else.
fn build_mock_env(vars: HashMap<&'static str, &str>) -> TestEnv {
    let vars: HashMap<String, String> = vars
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect();
    let mut env = MockEnv::new();
    env.expect_string()
        .times(0..)
        .returning(move |key| vars.get(key).cloned());
    TestEnv { inner: env }
}

/// Scenario: the TTL variable is absent, present and valid, or present and
/// unusable (non-numeric, below the minimum, above the maximum).
///
/// Invariant: the adapter resolves every case through the domain's clamp rules
/// without reading or mutating the process environment.
#[rstest]
#[case::absent(None, 24 * 3600)]
#[case::valid(Some("48"), 48 * 3600)]
#[case::non_numeric(Some("not_a_number"), 24 * 3600)]
#[case::below_minimum(Some("0"), 3600)]
#[case::above_maximum(Some("999999"), 87600 * 3600)]
fn idempotency_config_from_env_resolves_ttl(
    #[case] raw_value: Option<&str>,
    #[case] expected_seconds: u64,
) {
    let vars = raw_value.map_or_else(HashMap::new, |value| {
        HashMap::from([("IDEMPOTENCY_TTL_HOURS", value)])
    });

    let config = idempotency_config_from_env(&build_mock_env(vars));

    assert_eq!(
        config.ttl(),
        Duration::from_secs(expected_seconds),
        "the adapter must resolve the TTL through the domain's clamp rules"
    );
}
