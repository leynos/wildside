//! Diesel-backed `LoginService` adapter built on `DieselUserRepository`.
//!
//! This adapter preserves the fixture login contract (`admin`/`password`) while
//! ensuring the authenticated fixture user exists in PostgreSQL.

use std::sync::Arc;

use async_trait::async_trait;

use crate::domain::ports::{LoginService, UserRepository};
use crate::domain::{DisplayName, Error, LoginCredentials, User, UserId};

use super::diesel_user_repository::DieselUserRepository;
use super::user_persistence_error_mapping::map_user_persistence_error;
#[cfg(test)]
use crate::domain::ports::UserPersistenceError;

const FIXTURE_USERNAME: &str = "admin";
const FIXTURE_PASSWORD: &str = "password";
const FIXTURE_USER_ID: &str = "123e4567-e89b-12d3-a456-426614174000";
const FIXTURE_DISPLAY_NAME: &str = "Ada Lovelace";

/// Diesel-backed `LoginService` that preserves fixture-authentication semantics.
#[derive(Clone)]
pub struct DieselLoginService {
    user_repository: Arc<dyn UserRepository>,
}

impl DieselLoginService {
    /// Create a new service backed by a Diesel user repository.
    pub fn new(user_repository: DieselUserRepository) -> Self {
        Self {
            user_repository: Arc::new(user_repository),
        }
    }

    #[cfg(test)]
    fn from_repository(user_repository: Arc<dyn UserRepository>) -> Self {
        Self { user_repository }
    }

    async fn ensure_fixture_user_exists(&self, user_id: &UserId) -> Result<(), Error> {
        let existing = self
            .user_repository
            .find_by_id(user_id)
            .await
            .map_err(map_user_persistence_error)?;

        if existing.is_some() {
            return Ok(());
        }

        let display_name = DisplayName::new(FIXTURE_DISPLAY_NAME)
            .map_err(|err| Error::internal(format!("invalid fixture display name: {err}")))?;
        let user = User::new(user_id.clone(), display_name);

        self.user_repository
            .upsert(&user)
            .await
            .map_err(map_user_persistence_error)
    }
}

fn fixture_user_id() -> Result<UserId, Error> {
    UserId::new(FIXTURE_USER_ID)
        .map_err(|err| Error::internal(format!("invalid fixture user id: {err}")))
}

#[async_trait]
impl LoginService for DieselLoginService {
    async fn authenticate(&self, credentials: &LoginCredentials) -> Result<UserId, Error> {
        if credentials.username() != FIXTURE_USERNAME || credentials.password() != FIXTURE_PASSWORD
        {
            return Err(Error::unauthorized("invalid credentials"));
        }

        let user_id = fixture_user_id()?;
        self.ensure_fixture_user_exists(&user_id).await?;
        Ok(user_id)
    }
}

#[cfg(test)]
mod tests {
    //! Regression coverage for login fixture parity and persistence mapping.
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

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
        upsert_failure: Option<StubFailure>,
    }

    #[derive(Default)]
    struct StubUserRepository {
        state: Mutex<StubState>,
        upsert_calls: AtomicUsize,
    }

    impl StubUserRepository {
        fn with_user(user: User) -> Self {
            Self {
                state: Mutex::new(StubState {
                    stored_user: Some(user),
                    ..StubState::default()
                }),
                upsert_calls: AtomicUsize::new(0),
            }
        }

        fn set_find_failure(&self, failure: StubFailure) {
            self.state.lock().expect("state lock").find_failure = Some(failure);
        }

        fn set_upsert_failure(&self, failure: StubFailure) {
            self.state.lock().expect("state lock").upsert_failure = Some(failure);
        }

        fn upsert_call_count(&self) -> usize {
            self.upsert_calls.load(Ordering::Relaxed)
        }

        fn stored_user(&self) -> Option<User> {
            self.state.lock().expect("state lock").stored_user.clone()
        }
    }

    #[async_trait]
    impl UserRepository for StubUserRepository {
        async fn upsert(&self, user: &User) -> Result<(), UserPersistenceError> {
            self.upsert_calls.fetch_add(1, Ordering::Relaxed);
            let mut state = self.state.lock().expect("state lock");
            if let Some(failure) = state.upsert_failure {
                return Err(failure.to_error());
            }
            state.stored_user = Some(user.clone());
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

    fn credentials(username: &str, password: &str) -> LoginCredentials {
        LoginCredentials::try_from_parts(username, password).expect("valid test credentials")
    }

    fn fixture_user(display_name: &str) -> User {
        User::try_from_strings(FIXTURE_USER_ID, display_name).expect("valid fixture user")
    }

    #[tokio::test]
    async fn authenticate_with_fixture_credentials_creates_missing_user() {
        let repository = Arc::new(StubUserRepository::default());
        let service = DieselLoginService::from_repository(repository.clone());

        let user_id = service
            .authenticate(&credentials("admin", "password"))
            .await
            .expect("fixture credentials should authenticate");

        assert_eq!(user_id.as_ref(), FIXTURE_USER_ID);
        assert_eq!(repository.upsert_call_count(), 1);
        let stored = repository.stored_user().expect("user should be stored");
        assert_eq!(stored.id().as_ref(), FIXTURE_USER_ID);
        assert_eq!(stored.display_name().as_ref(), FIXTURE_DISPLAY_NAME);
    }

    #[tokio::test]
    async fn authenticate_keeps_existing_display_name_when_user_already_exists() {
        let existing_user = fixture_user("Existing Admin");
        let repository = Arc::new(StubUserRepository::with_user(existing_user));
        let service = DieselLoginService::from_repository(repository.clone());

        let _ = service
            .authenticate(&credentials("admin", "password"))
            .await
            .expect("fixture credentials should authenticate");

        assert_eq!(repository.upsert_call_count(), 0);
        let stored = repository
            .stored_user()
            .expect("existing user should remain");
        assert_eq!(stored.display_name().as_ref(), "Existing Admin");
    }

    #[rstest]
    #[case("admin", "wrong-password")]
    #[case("other-user", "password")]
    #[tokio::test]
    async fn authenticate_rejects_non_fixture_credentials(
        #[case] username: &str,
        #[case] password: &str,
    ) {
        let repository = Arc::new(StubUserRepository::default());
        let service = DieselLoginService::from_repository(repository.clone());

        let err = service
            .authenticate(&credentials(username, password))
            .await
            .expect_err("non fixture credentials must fail");

        assert_eq!(err.code(), ErrorCode::Unauthorized);
        assert_eq!(err.message(), "invalid credentials");
        assert_eq!(repository.upsert_call_count(), 0);
    }

    #[rstest]
    #[case(StubFailure::Connection, ErrorCode::ServiceUnavailable)]
    #[case(StubFailure::Query, ErrorCode::InternalError)]
    #[tokio::test]
    async fn authenticate_maps_find_errors(
        #[case] failure: StubFailure,
        #[case] expected_code: ErrorCode,
    ) {
        let repository = Arc::new(StubUserRepository::default());
        repository.set_find_failure(failure);
        let service = DieselLoginService::from_repository(repository);

        let err = service
            .authenticate(&credentials("admin", "password"))
            .await
            .expect_err("find failures should surface as domain errors");

        assert_eq!(err.code(), expected_code);
    }

    #[rstest]
    #[case(StubFailure::Connection, ErrorCode::ServiceUnavailable)]
    #[case(StubFailure::Query, ErrorCode::InternalError)]
    #[tokio::test]
    async fn authenticate_maps_upsert_errors(
        #[case] failure: StubFailure,
        #[case] expected_code: ErrorCode,
    ) {
        let repository = Arc::new(StubUserRepository::default());
        repository.set_upsert_failure(failure);
        let service = DieselLoginService::from_repository(repository);

        let err = service
            .authenticate(&credentials("admin", "password"))
            .await
            .expect_err("upsert failures should surface as domain errors");

        assert_eq!(err.code(), expected_code);
    }
}
