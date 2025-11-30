//! Authentication helpers used by HTTP handlers.
//!
//! Keep the HTTP modules focused on request/response mapping by concentrating
//! credential checks and user identity derivation here.

use crate::domain::{Error, LoginCredentials, UserId};

use super::ApiResult;

pub fn authenticate(credentials: &LoginCredentials) -> ApiResult<UserId> {
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
    use rstest::rstest;
    use rstest_bdd_macros::{given, then, when};

    #[given("valid admin credentials")]
    fn valid_admin_credentials() -> LoginCredentials {
        LoginCredentials::try_from_parts("admin", "password").expect("valid creds")
    }

    #[given("invalid credentials")]
    fn invalid_credentials() -> LoginCredentials {
        LoginCredentials::try_from_parts("admin", "wrong").expect("valid shape")
    }

    #[when("authentication runs")]
    fn authentication_runs(credentials: LoginCredentials) -> ApiResult<UserId> {
        authenticate(&credentials)
    }

    #[then("a user id is returned")]
    fn a_user_id_is_returned(result: ApiResult<UserId>) {
        assert!(result.is_ok(), "expected authentication success");
    }

    #[then("an unauthorised error is returned")]
    fn an_unauthorised_error_is_returned(result: ApiResult<UserId>) {
        let error = result.expect_err("should be an error");
        assert_eq!(error.code(), ErrorCode::Unauthorized);
    }

    #[rstest]
    fn authentication_happy_path() {
        let credentials = valid_admin_credentials();
        let result = authentication_runs(credentials);
        a_user_id_is_returned(result);
    }

    #[rstest]
    fn authentication_unhappy_path() {
        let credentials = invalid_credentials();
        let result = authentication_runs(credentials);
        an_unauthorised_error_is_returned(result);
    }
}
