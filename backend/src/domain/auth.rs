//! Authentication primitives such as login credentials.
//!
//! Keep inbound payload parsing outside the domain by exposing constructors
//! that validate string inputs before a handler talks to a port or service.

use std::fmt;

use zeroize::Zeroizing;

/// Domain error returned when login payload values are invalid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginValidationError {
    /// Username was missing or blank once trimmed.
    EmptyUsername,
    /// Password was blank.
    EmptyPassword,
}

impl fmt::Display for LoginValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyUsername => write!(f, "username must not be empty"),
            Self::EmptyPassword => write!(f, "password must not be empty"),
        }
    }
}

impl std::error::Error for LoginValidationError {}

/// Validated login credentials used by authentication services.
///
/// ## Invariants
/// - `username` is trimmed and must not be empty after trimming.
/// - `password` is required to be non-empty but retains caller-provided
///   whitespace to avoid surprising credential comparisons.
///
/// # Examples
/// ```
/// use backend::domain::LoginCredentials;
///
/// let creds = LoginCredentials::try_from_parts("admin", "password").unwrap();
/// assert_eq!(creds.username(), "admin");
/// assert_eq!(creds.password(), "password");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginCredentials {
    username: String,
    password: Zeroizing<String>,
}

impl LoginCredentials {
    /// Construct credentials from raw username/password inputs.
    pub fn try_from_parts(
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Result<Self, LoginValidationError> {
        let username = username.into();
        let normalized = username.trim();
        if normalized.is_empty() {
            return Err(LoginValidationError::EmptyUsername);
        }

        let password = password.into();
        if password.is_empty() {
            return Err(LoginValidationError::EmptyPassword);
        }

        Ok(Self {
            username: normalized.to_owned(),
            password: Zeroizing::new(password),
        })
    }

    /// Username string suitable for user lookups.
    pub fn username(&self) -> &str {
        self.username.as_str()
    }

    /// Password string provided by the caller.
    pub fn password(&self) -> &str {
        self.password.as_str()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::{fixture, rstest};
    use rstest_bdd_macros::{given, then, when};

    const VALID_USERNAME: &str = "admin@example.com";
    const VALID_PASSWORD: &str = "correct horse battery staple";

    #[fixture]
    fn username() -> String {
        VALID_USERNAME.into()
    }

    #[fixture]
    fn password() -> String {
        VALID_PASSWORD.into()
    }

    #[rstest]
    fn rejects_empty_username(password: String) {
        let result = LoginCredentials::try_from_parts("", password);
        assert!(matches!(result, Err(LoginValidationError::EmptyUsername)));
    }

    #[rstest]
    fn rejects_whitespace_username(password: String) {
        let result = LoginCredentials::try_from_parts("   ", password);
        assert!(matches!(result, Err(LoginValidationError::EmptyUsername)));
    }

    #[rstest]
    fn trims_username(password: String) {
        let creds = LoginCredentials::try_from_parts("  admin  ", password)
            .expect("username should be trimmed");
        assert_eq!(creds.username(), "admin");
    }

    #[rstest]
    fn rejects_empty_password(username: String) {
        let result = LoginCredentials::try_from_parts(username, "");
        assert!(matches!(result, Err(LoginValidationError::EmptyPassword)));
    }

    #[given("a valid login payload")]
    fn a_valid_login_payload(username: String, password: String) -> (String, String) {
        (username, password)
    }

    #[when("credentials are constructed")]
    fn credentials_are_constructed(
        payload: (String, String),
    ) -> Result<LoginCredentials, LoginValidationError> {
        LoginCredentials::try_from_parts(payload.0, payload.1)
    }

    #[then("the username is preserved")]
    fn the_username_is_preserved(result: Result<LoginCredentials, LoginValidationError>) {
        let creds = result.expect("credentials should be built");
        assert_eq!(creds.username(), VALID_USERNAME);
    }

    #[rstest]
    fn constructing_credentials_happy_path(username: String, password: String) {
        let payload = a_valid_login_payload(username, password);
        let result = credentials_are_constructed(payload);
        the_username_is_preserved(result);
    }

    #[given("a payload missing the password")]
    fn a_payload_missing_password(username: String) -> (String, String) {
        (username, String::new())
    }

    #[then("credential construction fails")]
    fn credential_construction_fails(result: Result<LoginCredentials, LoginValidationError>) {
        assert!(matches!(result, Err(LoginValidationError::EmptyPassword)));
    }

    #[rstest]
    fn constructing_credentials_unhappy_path(username: String) {
        let payload = a_payload_missing_password(username);
        let result = credentials_are_constructed(payload);
        credential_construction_fails(result);
    }
}
