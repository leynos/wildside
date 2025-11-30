//! Authentication helpers used by HTTP handlers.
//!
//! Keep the HTTP modules focused on request/response mapping by concentrating
//! credential checks and user identity derivation here.

use crate::domain::{Error, LoginCredentials, UserId};

use super::ApiResult;

pub fn authenticate(credentials: &LoginCredentials) -> ApiResult<UserId> {
    // TODO: Replace fixture-based authentication with a UserRepository lookup
    // once persistence is wired; keep handlers framework-agnostic.
    if credentials.username() == "admin" && credentials.password() == "password" {
        UserId::new("123e4567-e89b-12d3-a456-426614174000")
            .map_err(|err| Error::internal(format!("invalid fixture user id: {err}")))
    } else {
        Err(Error::unauthorized("invalid credentials"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ErrorCode;
    #[test]
    fn authentication_happy_path() {
        let credentials =
            LoginCredentials::try_from_parts("admin", "password").expect("valid creds");
        let result = authenticate(&credentials);
        assert!(result.is_ok(), "expected authentication success");
    }

    #[test]
    fn authentication_unhappy_path() {
        let credentials = LoginCredentials::try_from_parts("admin", "wrong").expect("valid shape");
        let result = authenticate(&credentials);
        let error = result.expect_err("should be an error");
        assert_eq!(error.code(), ErrorCode::Unauthorized);
    }
}
