//! Driving port for login/authentication use-cases.
//!
//! In hexagonal terms this is a *driving* port: inbound adapters call it to
//! authenticate credentials without knowing (or importing) the backing
//! infrastructure. This makes HTTP handler tests deterministic because they
//! can substitute a test double instead of wiring persistence.

use async_trait::async_trait;

use crate::domain::{Error, LoginCredentials, UserId};

/// Domain use-case port for authentication.
#[async_trait]
pub trait LoginService: Send + Sync {
    /// Validate credentials and return the authenticated user id.
    async fn authenticate(&self, credentials: &LoginCredentials) -> Result<UserId, Error>;
}

/// Temporary in-memory authenticator used until persistence is wired.
///
/// This preserves the existing development behaviour:
/// `admin` / `password` authenticates successfully and produces a fixed user id.
#[derive(Debug, Default, Clone, Copy)]
pub struct FixtureLoginService;

#[async_trait]
impl LoginService for FixtureLoginService {
    async fn authenticate(&self, credentials: &LoginCredentials) -> Result<UserId, Error> {
        if credentials.username() == "admin" && credentials.password() == "password" {
            UserId::new("123e4567-e89b-12d3-a456-426614174000")
                .map_err(|err| Error::internal(format!("invalid fixture user id: {err}")))
        } else {
            Err(Error::unauthorized("invalid credentials"))
        }
    }
}

#[cfg(test)]
mod tests {
    //! Regression coverage for this module.
    use super::*;
    use crate::domain::ErrorCode;
    use rstest::rstest;

    #[rstest]
    #[case("admin", "password", true)]
    #[case("admin", "wrong", false)]
    #[case("other", "password", false)]
    #[tokio::test]
    async fn fixture_login_service_behaves_like_existing_handler(
        #[case] username: &str,
        #[case] password: &str,
        #[case] should_succeed: bool,
    ) {
        let service = FixtureLoginService;
        let creds =
            LoginCredentials::try_from_parts(username, password).expect("credentials shape");
        let result = service.authenticate(&creds).await;
        match (should_succeed, result) {
            (true, Ok(id)) => assert_eq!(id.as_ref(), "123e4567-e89b-12d3-a456-426614174000"),
            (false, Err(err)) => assert_eq!(err.code(), ErrorCode::Unauthorized),
            (true, Err(err)) => panic!("expected success, got error: {err:?}"),
            (false, Ok(id)) => panic!("expected failure, got success: {id}"),
        }
    }
}
