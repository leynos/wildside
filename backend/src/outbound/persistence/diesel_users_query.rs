//! Diesel-backed `UsersQuery` adapter built on `DieselUserRepository`.
//!
//! This adapter fetches user records from PostgreSQL for the authenticated
//! session subject.

use std::sync::Arc;

use async_trait::async_trait;
use pagination::Direction;

use crate::domain::ports::{ListUsersPageRequest, UserRepository, UsersPage, UsersQuery};
use crate::domain::{Error, User, UserCursorKey, UserId};

use super::diesel_user_repository::DieselUserRepository;
use super::user_persistence_error_mapping::map_user_persistence_error;
#[cfg(test)]
use crate::domain::ports::UserPersistenceError;

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

#[async_trait]
impl UsersQuery for DieselUsersQuery {
    async fn list_users(&self, authenticated_user: &UserId) -> Result<Vec<User>, Error> {
        let maybe_user = self
            .user_repository
            .find_by_id(authenticated_user)
            .await
            .map_err(map_user_persistence_error)?;

        match maybe_user {
            Some(user) => Ok(vec![user]),
            None => Ok(Vec::new()),
        }
    }

    async fn list_users_page(
        &self,
        _authenticated_user: &UserId,
        request: ListUsersPageRequest,
    ) -> Result<UsersPage, Error> {
        let direction = page_direction(&request);
        let limit = request.limit();
        let mut rows = self
            .user_repository
            .list_page(request)
            .await
            .map_err(map_user_persistence_error)?;

        let has_more = rows.len() > limit;
        if has_more {
            trim_overflow_row(&mut rows, limit, direction);
        }

        Ok(UsersPage::new(rows, has_more))
    }
}

fn page_direction(request: &ListUsersPageRequest) -> Direction {
    request.cursor().map_or(
        Direction::Next,
        pagination::Cursor::<UserCursorKey>::direction,
    )
}

fn trim_overflow_row(rows: &mut Vec<User>, limit: usize, direction: Direction) {
    match direction {
        Direction::Next => rows.truncate(limit),
        Direction::Prev => {
            let overflow = rows.len().saturating_sub(limit);
            rows.drain(0..overflow);
        }
    }
}

#[cfg(test)]
mod tests {
    //! Regression coverage for users query mapping and response shape.
    use std::{error::Error as StdError, sync::Mutex};

    use super::*;
    use crate::domain::ErrorCode;
    use chrono::{DateTime, Utc};
    use pagination::Cursor;
    use rstest::rstest;

    type TestResult<T = ()> = Result<T, Box<dyn StdError>>;

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
        page_rows: Vec<User>,
        list_failure: Option<StubFailure>,
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

        fn with_page_rows(page_rows: Vec<User>) -> Self {
            Self {
                state: Mutex::new(StubState {
                    page_rows,
                    ..StubState::default()
                }),
            }
        }

        fn set_find_failure(&self, failure: StubFailure) -> Result<(), UserPersistenceError> {
            self.state
                .lock()
                .map_err(|_| UserPersistenceError::query("state lock"))?
                .find_failure = Some(failure);
            Ok(())
        }

        fn set_list_failure(&self, failure: StubFailure) -> Result<(), UserPersistenceError> {
            self.state
                .lock()
                .map_err(|_| UserPersistenceError::query("state lock"))?
                .list_failure = Some(failure);
            Ok(())
        }
    }

    #[async_trait]
    impl UserRepository for StubUserRepository {
        async fn upsert(&self, _user: &User) -> Result<(), UserPersistenceError> {
            Ok(())
        }

        async fn find_by_id(&self, id: &UserId) -> Result<Option<User>, UserPersistenceError> {
            let state = self
                .state
                .lock()
                .map_err(|_| UserPersistenceError::query("state lock"))?;
            if let Some(failure) = state.find_failure {
                return Err(failure.to_error());
            }
            Ok(state
                .stored_user
                .as_ref()
                .filter(|user| user.id() == id)
                .cloned())
        }

        async fn list_page(
            &self,
            _request: ListUsersPageRequest,
        ) -> Result<Vec<User>, UserPersistenceError> {
            let state = self
                .state
                .lock()
                .map_err(|_| UserPersistenceError::query("state lock"))?;
            if let Some(failure) = state.list_failure {
                return Err(failure.to_error());
            }
            Ok(state.page_rows.clone())
        }
    }

    fn user_id(id: &str) -> TestResult<UserId> {
        Ok(UserId::new(id)?)
    }

    fn user(id: &str, display_name: &str) -> TestResult<User> {
        Ok(User::try_from_strings(id, display_name)?)
    }

    fn timestamp(value: &str) -> TestResult<DateTime<Utc>> {
        Ok(DateTime::parse_from_rfc3339(value)?.with_timezone(&Utc))
    }

    fn user_at(id: &str, display_name: &str, created_at: &str) -> TestResult<User> {
        Ok(User::try_from_strings_at(
            id,
            display_name,
            timestamp(created_at)?,
        )?)
    }

    fn request_with_cursor(
        user: &User,
        direction: Direction,
        limit: usize,
    ) -> ListUsersPageRequest {
        ListUsersPageRequest::new(
            Some(Cursor::with_direction(UserCursorKey::from(user), direction)),
            limit,
        )
    }

    #[tokio::test]
    async fn list_users_returns_authenticated_user_when_present() -> TestResult {
        let auth_user = user("11111111-1111-1111-1111-111111111111", "Ada Lovelace")?;
        let authenticated_user_id = auth_user.id().clone();
        let repository = Arc::new(StubUserRepository::with_user(auth_user.clone()));
        let query = DieselUsersQuery::from_repository(repository);

        let users = query.list_users(&authenticated_user_id).await?;

        assert_eq!(users, vec![auth_user]);
        Ok(())
    }

    #[tokio::test]
    async fn list_users_returns_empty_list_when_authenticated_user_missing() -> TestResult {
        let repository = Arc::new(StubUserRepository::default());
        let query = DieselUsersQuery::from_repository(repository);

        let users = query
            .list_users(&user_id("11111111-1111-1111-1111-111111111111")?)
            .await?;

        assert!(users.is_empty());
        Ok(())
    }

    #[rstest]
    #[case(StubFailure::Connection, ErrorCode::ServiceUnavailable)]
    #[case(StubFailure::Query, ErrorCode::InternalError)]
    #[tokio::test]
    async fn list_users_maps_persistence_failures(
        #[case] failure: StubFailure,
        #[case] expected_code: ErrorCode,
    ) -> TestResult {
        let repository = Arc::new(StubUserRepository::default());
        repository.set_find_failure(failure)?;
        let query = DieselUsersQuery::from_repository(repository);

        let err = query
            .list_users(&user_id("11111111-1111-1111-1111-111111111111")?)
            .await
            .expect_err("repository failures should map to domain errors");

        assert_eq!(err.code(), expected_code);
        Ok(())
    }

    #[tokio::test]
    async fn list_users_page_trims_forward_overflow_row() -> TestResult {
        let rows = vec![
            user_at(
                "11111111-1111-1111-1111-111111111111",
                "Ada One",
                "2026-01-01T00:00:00Z",
            )?,
            user_at(
                "22222222-2222-2222-2222-222222222222",
                "Ada Two",
                "2026-01-02T00:00:00Z",
            )?,
            user_at(
                "33333333-3333-3333-3333-333333333333",
                "Ada Three",
                "2026-01-03T00:00:00Z",
            )?,
        ];
        let repository = Arc::new(StubUserRepository::with_page_rows(rows.clone()));
        let query = DieselUsersQuery::from_repository(repository);

        let page = query
            .list_users_page(
                rows[0].id(),
                ListUsersPageRequest::new(None, rows.len() - 1),
            )
            .await?;

        assert_eq!(page.rows(), &rows[0..2]);
        assert!(page.has_more());
        Ok(())
    }

    #[tokio::test]
    async fn list_users_page_trims_reverse_overflow_row() -> TestResult {
        let rows = vec![
            user_at(
                "11111111-1111-1111-1111-111111111111",
                "Overflow",
                "2026-01-01T00:00:00Z",
            )?,
            user_at(
                "22222222-2222-2222-2222-222222222222",
                "Ada Two",
                "2026-01-02T00:00:00Z",
            )?,
            user_at(
                "33333333-3333-3333-3333-333333333333",
                "Ada Three",
                "2026-01-03T00:00:00Z",
            )?,
        ];
        let repository = Arc::new(StubUserRepository::with_page_rows(rows.clone()));
        let query = DieselUsersQuery::from_repository(repository);
        let request = request_with_cursor(&rows[2], Direction::Prev, rows.len() - 1);

        let page = query.list_users_page(rows[2].id(), request).await?;

        assert_eq!(page.rows(), &rows[1..3]);
        assert!(page.has_more());
        Ok(())
    }

    #[rstest]
    #[case(StubFailure::Connection, ErrorCode::ServiceUnavailable)]
    #[case(StubFailure::Query, ErrorCode::InternalError)]
    #[tokio::test]
    async fn list_users_page_maps_persistence_failures(
        #[case] failure: StubFailure,
        #[case] expected_code: ErrorCode,
    ) -> TestResult {
        let repository = Arc::new(StubUserRepository::default());
        repository.set_list_failure(failure)?;
        let query = DieselUsersQuery::from_repository(repository);

        let err = query
            .list_users_page(
                &user_id("11111111-1111-1111-1111-111111111111")?,
                ListUsersPageRequest::new(None, 20),
            )
            .await
            .expect_err("repository failures should map to domain errors");

        assert_eq!(err.code(), expected_code);
        Ok(())
    }
}
