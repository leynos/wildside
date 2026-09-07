//! Helpers for controlling embedded test cluster behaviour.
//!
//! Integration tests that depend on embedded PostgreSQL can be optionally
//! skipped in environments where the cluster cannot be started. This module
//! centralizes the `SKIP_TEST_CLUSTER` policy and error messaging so all test
//! suites behave consistently.

/// Returns true when the `SKIP_TEST_CLUSTER` environment variable is set to a
/// truthy value.
///
/// Reads through `super::process_env`, the test-support tree's single ambient
/// environment read; [`should_skip_test_cluster_with`] holds the policy and is
/// tested with a stub reader.
pub fn should_skip_test_cluster() -> bool {
    should_skip_test_cluster_with(super::process_env)
}

/// Decide the skip policy from an injected environment reader.
///
/// Truthy values: "1", "true", "yes" (case-insensitive, surrounding whitespace
/// ignored).
pub fn should_skip_test_cluster_with(read_env: impl Fn(&str) -> Option<String>) -> bool {
    read_env("SKIP_TEST_CLUSTER").is_some_and(|value| {
        let value = value.trim().to_ascii_lowercase();
        matches!(value.as_str(), "1" | "true" | "yes")
    })
}

/// Handles embedded cluster setup failures consistently across integration tests.
///
/// When `SKIP_TEST_CLUSTER` is truthy, prints a skip marker and returns `None`.
/// Otherwise, panics with a clear failure message so CI breakage is not masked.
pub fn handle_cluster_setup_failure<T>(reason: impl std::fmt::Display) -> Option<T> {
    if should_skip_test_cluster() {
        eprintln!("SKIP-TEST-CLUSTER: {reason}");
        None
    } else {
        panic!("Test cluster setup failed: {reason}. Set SKIP_TEST_CLUSTER=1 to skip.");
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for the embedded-cluster skip policy.

    use super::should_skip_test_cluster_with;
    use rstest::rstest;

    /// Scenario: `SKIP_TEST_CLUSTER` is absent or set to a truthy, padded, or
    /// unrecognized value.
    ///
    /// Invariant: only the documented truthy spellings skip the suite, and the
    /// decision is made entirely from the injected reader.
    #[rstest]
    #[case::absent(None, false)]
    #[case::one(Some("1"), true)]
    #[case::mixed_case_true(Some("tRue"), true)]
    #[case::padded_yes(Some("  yes  "), true)]
    #[case::empty(Some(""), false)]
    #[case::unrecognized(Some("maybe"), false)]
    fn skip_policy_reads_only_the_injected_value(
        #[case] value: Option<&'static str>,
        #[case] expected: bool,
    ) {
        let skipped = should_skip_test_cluster_with(|name| {
            (name == "SKIP_TEST_CLUSTER")
                .then(|| value.map(str::to_owned))
                .flatten()
        });

        assert_eq!(
            skipped, expected,
            "the skip decision must follow the injected value alone"
        );
    }
}
