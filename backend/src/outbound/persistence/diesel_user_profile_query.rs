//! Diesel-backed `UserProfileQuery` adapter built on a user repository.
//!
//! This adapter resolves an authenticated user's profile from PostgreSQL while
//! keeping fixture-compatible error mapping semantics.

use std::sync::Arc;

use async_trait::async_trait;

#[cfg(test)]
use crate::domain::ports::UserPersistenceError;
use crate::domain::ports::{UserProfileQuery, UserRepository};
use crate::domain::{Error, User, UserId};

use super::user_persistence_error_mapping::map_user_persistence_error;

/// Diesel-backed `UserProfileQuery` implementation.
#[derive(Clone)]
pub struct DieselUserProfileQuery {
    user_repository: Arc<dyn UserRepository>,
}

impl DieselUserProfileQuery {
    /// Create a new profile query adapter backed by a user repository.
    pub fn new(user_repository: Arc<dyn UserRepository>) -> Self {
        Self { user_repository }
    }
}

#[async_trait]
impl UserProfileQuery for DieselUserProfileQuery {
    async fn fetch_profile(&self, user_id: &UserId) -> Result<User, Error> {
        let maybe_user = self
            .user_repository
            .find_by_id(user_id)
            .await
            .map_err(map_user_persistence_error)?;

        match maybe_user {
            Some(user) => Ok(user),
            None => Err(Error::internal(format!(
                "authenticated user record is missing for id {}",
                user_id.as_ref()
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    //! Regression coverage for profile lookup and error mapping.
    use super::*;
    use crate::domain::ErrorCode;
    use rstest::rstest;
    use std::error::Error as StdError;

    type TestResult<T = ()> = Result<T, Box<dyn StdError>>;

    struct SuccessUserRepository {
        user: User,
    }

    struct MissingUserRepository;

    #[derive(Clone)]
    struct FailingUserRepository {
        error: UserPersistenceError,
    }

    #[async_trait]
    impl UserRepository for SuccessUserRepository {
        async fn upsert(&self, _user: &User) -> Result<(), UserPersistenceError> {
            Ok(())
        }

        async fn find_by_id(&self, id: &UserId) -> Result<Option<User>, UserPersistenceError> {
            Ok((self.user.id() == id).then(|| self.user.clone()))
        }
    }

    #[async_trait]
    impl UserRepository for MissingUserRepository {
        async fn upsert(&self, _user: &User) -> Result<(), UserPersistenceError> {
            Ok(())
        }

        async fn find_by_id(&self, _id: &UserId) -> Result<Option<User>, UserPersistenceError> {
            Ok(None)
        }
    }

    #[async_trait]
    impl UserRepository for FailingUserRepository {
        async fn upsert(&self, _user: &User) -> Result<(), UserPersistenceError> {
            Ok(())
        }

        async fn find_by_id(&self, _id: &UserId) -> Result<Option<User>, UserPersistenceError> {
            Err(self.error.clone())
        }
    }

    fn user_id(value: &str) -> TestResult<UserId> {
        Ok(UserId::new(value)?)
    }

    fn user(id: &str, display_name: &str) -> TestResult<User> {
        Ok(User::try_from_strings(id, display_name)?)
    }

    #[tokio::test]
    async fn fetch_profile_returns_user_when_present() -> TestResult {
        let profile_user = user("11111111-1111-1111-1111-111111111111", "Ada Lovelace")?;
        let query = DieselUserProfileQuery::new(Arc::new(SuccessUserRepository {
            user: profile_user.clone(),
        }));

        let profile = query.fetch_profile(profile_user.id()).await?;

        assert_eq!(profile, profile_user);
        Ok(())
    }

    #[tokio::test]
    async fn fetch_profile_returns_internal_error_when_user_missing() -> TestResult {
        let query = DieselUserProfileQuery::new(Arc::new(MissingUserRepository));
        let authenticated_user = user_id("11111111-1111-1111-1111-111111111111")?;

        let err = query
            .fetch_profile(&authenticated_user)
            .await
            .expect_err("missing profile should map to error");

        assert_eq!(err.code(), ErrorCode::InternalError);
        assert!(err.message().contains(authenticated_user.as_ref()));
        Ok(())
    }

    #[rstest]
    #[case(
        UserPersistenceError::connection("database unavailable"),
        ErrorCode::ServiceUnavailable
    )]
    #[case(
        UserPersistenceError::query("database query failed"),
        ErrorCode::InternalError
    )]
    #[tokio::test]
    async fn fetch_profile_maps_persistence_failures(
        #[case] failure: UserPersistenceError,
        #[case] expected_code: ErrorCode,
    ) -> TestResult {
        let query = DieselUserProfileQuery::new(Arc::new(FailingUserRepository { error: failure }));

        let err = query
            .fetch_profile(&user_id("11111111-1111-1111-1111-111111111111")?)
            .await
            .expect_err("repository failures should map to domain errors");

        assert_eq!(err.code(), expected_code);
        Ok(())
    }
}
