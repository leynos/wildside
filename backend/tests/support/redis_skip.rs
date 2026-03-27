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
    if is_env_truthy("SKIP_REDIS_TESTS") {
        return true;
    }

    !redis_server_is_available()
}

fn is_env_truthy(var: &str) -> bool {
    std::env::var(var)
        .map(|value| {
            let value = value.trim().to_ascii_lowercase();
            matches!(value.as_str(), "1" | "true" | "yes")
        })
        .unwrap_or(false)
}

fn redis_server_is_available() -> bool {
    std::process::Command::new("redis-server")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}
