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
    pub fn try_from_parts(username: &str, password: &str) -> Result<Self, LoginValidationError> {
        let normalized = username.trim();
        if normalized.is_empty() {
            return Err(LoginValidationError::EmptyUsername);
        }

        if password.is_empty() {
            return Err(LoginValidationError::EmptyPassword);
        }

        Ok(Self {
            username: normalized.to_owned(),
            password: Zeroizing::new(password.to_owned()),
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
    //! Regression coverage for this module.
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("", "pw", LoginValidationError::EmptyUsername)]
    #[case("   ", "pw", LoginValidationError::EmptyUsername)]
    #[case("user", "", LoginValidationError::EmptyPassword)]
    fn invalid_credentials(
        #[case] username: &str,
        #[case] password: &str,
        #[case] expected: LoginValidationError,
    ) {
        let err = LoginCredentials::try_from_parts(username, password)
            .expect_err("invalid inputs must fail");
        assert_eq!(err, expected);
    }

    #[rstest]
    #[case("  admin  ", "secret")]
    #[case("alice", "correct horse battery staple")]
    fn valid_credentials_trim_username(#[case] username: &str, #[case] password: &str) {
        let creds = LoginCredentials::try_from_parts(username, password)
            .expect("valid inputs should succeed");
        assert_eq!(creds.username(), username.trim());
        assert_eq!(creds.password(), password);
    }
}
