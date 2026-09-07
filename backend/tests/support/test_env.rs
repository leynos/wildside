//! The test-support tree's single ambient environment read.
//!
//! Integration binaries compile as separate crates, so this file is pulled in
//! by `#[path]` wherever a support module needs to consult the process. Keeping
//! it to one function keeps the composition-root exception in one place: every
//! helper that depends on an environment value takes a reader (or the resolved
//! value) as an argument and is exercised with a stub.
//!
//! Nothing in `backend/tests/` writes the environment. Values that a
//! third-party library reads for itself are composed by the runner; see
//! "Environment seams" in `docs/developers-guide.md`.

/// Read one variable from the test process's environment.
///
/// Values are read as UTF-8. A non-UTF-8 value is reported as absent, which is
/// the same outcome as leaving the variable unset.
#[expect(
    clippy::disallowed_methods,
    reason = "test-binary composition root: the one ambient read in \
              backend/tests/support, injected into every helper below it"
)]
pub fn process_env(name: &str) -> Option<String> {
    std::env::var(name).ok()
}
