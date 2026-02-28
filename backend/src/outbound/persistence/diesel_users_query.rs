//! Diesel-backed `UsersQuery` adapter built on `DieselUserRepository`.
//!
//! This adapter fetches user records from PostgreSQL for the authenticated
//! session subject.

use std::sync::Arc;

use async_trait::async_trait;

use crate::domain::ports::{UserPersistenceError, UserRepository, UsersQuery};
use crate::domain::{Error, User, UserId};

use super::diesel_user_repository::DieselUserRepository;

/// Diesel-backed `UsersQuery` implementation backed by user repository reads.
#[derive(Clone)]
pub struct DieselUsersQuery {
    user_repository: Arc<dyn UserRepository>,
}

impl DieselUsersQuery {
    /// Create a new query adapter backed by a Diesel user repository.
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

fn map_persistence_error(error: UserPersistenceError) -> Error {
    match error {
        UserPersistenceError::Connection { message } => Error::service_unavailable(message),
        UserPersistenceError::Query { message } => Error::internal(message),
    }
}

#[async_trait]
impl UsersQuery for DieselUsersQuery {
    async fn list_users(&self, authenticated_user: &UserId) -> Result<Vec<User>, Error> {
        let maybe_user = self
            .user_repository
            .find_by_id(authenticated_user)
            .await
            .map_err(map_persistence_error)?;

        match maybe_user {
            Some(user) => Ok(vec![user]),
            None => Ok(Vec::new()),
        }
    }
}

#[cfg(test)]
mod tests {
    //! Regression coverage for users query mapping and response shape.
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
        fn with_user(user: User) -> Self {
            Self {
                state: Mutex::new(StubState {
                    stored_user: Some(user),
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

    fn user_id(id: &str) -> UserId {
        UserId::new(id).expect("valid user id")
    }

    fn user(id: &str, display_name: &str) -> User {
        User::try_from_strings(id, display_name).expect("valid user")
    }

    #[tokio::test]
    async fn list_users_returns_authenticated_user_when_present() {
        let auth_user = user("11111111-1111-1111-1111-111111111111", "Ada Lovelace");
        let authenticated_user_id = auth_user.id().clone();
        let repository = Arc::new(StubUserRepository::with_user(auth_user.clone()));
        let query = DieselUsersQuery::from_repository(repository);

        let users = query
            .list_users(&authenticated_user_id)
            .await
            .expect("query should succeed");

        assert_eq!(users, vec![auth_user]);
    }

    #[tokio::test]
    async fn list_users_returns_empty_list_when_authenticated_user_missing() {
        let repository = Arc::new(StubUserRepository::default());
        let query = DieselUsersQuery::from_repository(repository);

        let users = query
            .list_users(&user_id("11111111-1111-1111-1111-111111111111"))
            .await
            .expect("query should succeed");

        assert!(users.is_empty());
    }

    #[rstest]
    #[case(StubFailure::Connection, ErrorCode::ServiceUnavailable)]
    #[case(StubFailure::Query, ErrorCode::InternalError)]
    #[tokio::test]
    async fn list_users_maps_persistence_failures(
        #[case] failure: StubFailure,
        #[case] expected_code: ErrorCode,
    ) {
        let repository = Arc::new(StubUserRepository::default());
        repository.set_find_failure(failure);
        let query = DieselUsersQuery::from_repository(repository);

        let err = query
            .list_users(&user_id("11111111-1111-1111-1111-111111111111"))
            .await
            .expect_err("repository failures should map to domain errors");

        assert_eq!(err.code(), expected_code);
    }
}
