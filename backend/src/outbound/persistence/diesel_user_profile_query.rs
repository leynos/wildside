//! Diesel-backed `UserProfileQuery` adapter built on `DieselUserRepository`.
//!
//! This adapter resolves an authenticated user's profile from PostgreSQL while
//! keeping fixture-compatible error mapping semantics.

use std::sync::Arc;

use async_trait::async_trait;

#[cfg(test)]
use crate::domain::ports::UserPersistenceError;
use crate::domain::ports::{UserProfileQuery, UserRepository};
use crate::domain::{Error, User, UserId};

use super::diesel_user_repository::DieselUserRepository;
use super::user_persistence_error_mapping::map_user_persistence_error;

/// Diesel-backed `UserProfileQuery` implementation.
#[derive(Clone)]
pub struct DieselUserProfileQuery {
    user_repository: Arc<dyn UserRepository>,
}

impl DieselUserProfileQuery {
    /// Create a new profile query adapter backed by a Diesel user repository.
    pub fn new(user_repository: DieselUserRepository) -> Self {
        Self {
            user_repository: Arc::new(user_repository),
        }
    }

    #[cfg(test)]
    fn from_repository(user_repository: Arc<dyn UserRepository>) -> Self {
        Self { user_repository }
    }
}

fn missing_user_error(user_id: &UserId) -> Error {
    Error::internal(format!(
        "authenticated user record is missing for id {}",
        user_id.as_ref()
    ))
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
            None => Err(missing_user_error(user_id)),
        }
    }
}

#[cfg(test)]
mod tests {
    //! Regression coverage for profile lookup and error mapping.
    use std::sync::Mutex;

    use super::*;
    use crate::domain::ErrorCode;
    use rstest::rstest;

    #[derive(Clone, Copy)]
    enum StubFailure {
        Connection,
        Query,
    }

    impl StubFailure {
        fn to_error(self) -> UserPersistenceError {
            match self {
                Self::Connection => UserPersistenceError::connection("database unavailable"),
                Self::Query => UserPersistenceError::query("database query failed"),
            }
        }
    }

    #[derive(Default)]
    struct StubState {
        stored_user: Option<User>,
        find_failure: Option<StubFailure>,
    }

    #[derive(Default)]
    struct StubUserRepository {
        state: Mutex<StubState>,
    }

    impl StubUserRepository {
        fn with_user(stored_user: User) -> Self {
            Self {
                state: Mutex::new(StubState {
                    stored_user: Some(stored_user),
                    ..StubState::default()
                }),
            }
        }

        fn set_find_failure(&self, failure: StubFailure) {
            self.state.lock().expect("state lock").find_failure = Some(failure);
        }
    }

    #[async_trait]
    impl UserRepository for StubUserRepository {
        async fn upsert(&self, _user: &User) -> Result<(), UserPersistenceError> {
            Ok(())
        }

        async fn find_by_id(&self, id: &UserId) -> Result<Option<User>, UserPersistenceError> {
            let state = self.state.lock().expect("state lock");
            if let Some(failure) = state.find_failure {
                return Err(failure.to_error());
            }

            Ok(state
                .stored_user
                .as_ref()
                .filter(|user| user.id() == id)
                .cloned())
        }
    }

    fn user_id(value: &str) -> UserId {
        UserId::new(value).expect("valid user id")
    }

    fn user(id: &str, display_name: &str) -> User {
        User::try_from_strings(id, display_name).expect("valid user")
    }

    #[tokio::test]
    async fn fetch_profile_returns_user_when_present() {
        let profile_user = user("11111111-1111-1111-1111-111111111111", "Ada Lovelace");
        let repository = Arc::new(StubUserRepository::with_user(profile_user.clone()));
        let query = DieselUserProfileQuery::from_repository(repository);

        let profile = query
            .fetch_profile(profile_user.id())
            .await
            .expect("profile should load");

        assert_eq!(profile, profile_user);
    }

    #[tokio::test]
    async fn fetch_profile_returns_internal_error_when_user_missing() {
        let query =
            DieselUserProfileQuery::from_repository(Arc::new(StubUserRepository::default()));
        let authenticated_user = user_id("11111111-1111-1111-1111-111111111111");

        let err = query
            .fetch_profile(&authenticated_user)
            .await
            .expect_err("missing profile should map to error");

        assert_eq!(err.code(), ErrorCode::InternalError);
        assert!(err.message().contains(authenticated_user.as_ref()));
    }

    #[rstest]
    #[case(StubFailure::Connection, ErrorCode::ServiceUnavailable)]
    #[case(StubFailure::Query, ErrorCode::InternalError)]
    #[tokio::test]
    async fn fetch_profile_maps_persistence_failures(
        #[case] failure: StubFailure,
        #[case] expected_code: ErrorCode,
    ) {
        let repository = Arc::new(StubUserRepository::default());
        repository.set_find_failure(failure);
        let query = DieselUserProfileQuery::from_repository(repository);

        let err = query
            .fetch_profile(&user_id("11111111-1111-1111-1111-111111111111"))
            .await
            .expect_err("repository failures should map to domain errors");

        assert_eq!(err.code(), expected_code);
    }
}
