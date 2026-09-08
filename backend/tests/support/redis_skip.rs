//! Helpers for skipping Redis-dependent integration tests.
//!
//! Mirrors the pattern established by [`super::cluster_skip`] for embedded
//! PostgreSQL. Tests call [`should_skip_redis_tests`] to decide at runtime
//! whether to execute or early-return when `redis-server` is absent or
//! `SKIP_REDIS_TESTS` is set.

/// Returns true when Redis-dependent tests should be skipped.
///
/// Skip when:
/// - The `SKIP_REDIS_TESTS` environment variable is set to a truthy value
///   ("1", "true", "yes", case-insensitive), OR
/// - The `redis-server` binary is not found on `PATH`.
pub fn should_skip_redis_tests() -> bool {
    if is_env_truthy(super::process_env, "SKIP_REDIS_TESTS") {
        return true;
    }

    !redis_server_is_available()
}

/// Decide whether a variable read through `read_env` is truthy.
///
/// Truthy values: "1", "true", "yes" (case-insensitive, surrounding whitespace
/// ignored). Taking the reader keeps the policy testable without process
/// mutation.
fn is_env_truthy(read_env: impl Fn(&str) -> Option<String>, var: &str) -> bool {
    read_env(var).is_some_and(|value| {
        let value = value.trim().to_ascii_lowercase();
        matches!(value.as_str(), "1" | "true" | "yes")
    })
}

/// Report whether a `redis-server` binary can be executed from `PATH`.
pub fn redis_server_is_available() -> bool {
    std::process::Command::new("redis-server")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

#[cfg(test)]
mod tests {
    //! Unit tests for the Redis skip policy's truthiness rule.

    use super::is_env_truthy;
    use rstest::rstest;

    /// Scenario: `SKIP_REDIS_TESTS` is absent or set to a truthy, padded, or
    /// unrecognized value.
    ///
    /// Invariant: only the documented truthy spellings count, and the decision
    /// is made entirely from the injected reader.
    #[rstest]
    #[case::absent(None, false)]
    #[case::one(Some("1"), true)]
    #[case::mixed_case_true(Some("tRue"), true)]
    #[case::padded_yes(Some("  yes  "), true)]
    #[case::empty(Some(""), false)]
    #[case::unrecognized(Some("maybe"), false)]
    fn truthiness_reads_only_the_injected_value(
        #[case] value: Option<&'static str>,
        #[case] expected: bool,
    ) {
        let truthy = is_env_truthy(
            |name| {
                (name == "SKIP_REDIS_TESTS")
                    .then(|| value.map(str::to_owned))
                    .flatten()
            },
            "SKIP_REDIS_TESTS",
        );

        assert_eq!(
            truthy, expected,
            "the truthiness decision must follow the injected value alone"
        );
    }
}
