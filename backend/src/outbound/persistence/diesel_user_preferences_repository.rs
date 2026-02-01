//! PostgreSQL-backed `UserPreferencesRepository` implementation using Diesel ORM.
//!
//! This adapter implements the domain's `UserPreferencesRepository` port, providing
//! durable storage for user preferences with optimistic concurrency support via
//! revision checks.

use async_trait::async_trait;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use tracing::debug;

use crate::domain::ports::{UserPreferencesRepository, UserPreferencesRepositoryError};
use crate::domain::{UnitSystem, UserId, UserPreferences};

use super::models::{NewUserPreferencesRow, UserPreferencesRow, UserPreferencesUpdate};
use super::pool::{DbPool, PoolError};
use super::schema::user_preferences;

/// Diesel-backed implementation of the `UserPreferencesRepository` port.
///
/// Provides PostgreSQL persistence for user preferences, supporting optimistic
/// concurrency via revision checks. Each save operation either inserts new
/// preferences or updates existing ones with a revision check.
#[derive(Clone)]
pub struct DieselUserPreferencesRepository {
    pool: DbPool,
}

impl DieselUserPreferencesRepository {
    /// Create a new repository with the given connection pool.
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

/// Map pool errors to domain user preferences repository errors.
fn map_pool_error(error: PoolError) -> UserPreferencesRepositoryError {
    match error {
        PoolError::Checkout { message } | PoolError::Build { message } => {
            UserPreferencesRepositoryError::connection(message)
        }
    }
}

/// Map Diesel errors to domain user preferences repository errors.
fn map_diesel_error(error: diesel::result::Error) -> UserPreferencesRepositoryError {
    use diesel::result::{DatabaseErrorKind, Error as DieselError};

    match &error {
        DieselError::DatabaseError(kind, info) => {
            debug!(?kind, message = info.message(), "diesel operation failed");
        }
        _ => debug!(
            error_type = %std::any::type_name_of_val(&error),
            "diesel operation failed"
        ),
    }

    match error {
        DieselError::NotFound => UserPreferencesRepositoryError::query("record not found"),
        DieselError::QueryBuilderError(_) => {
            UserPreferencesRepositoryError::query("database query error")
        }
        DieselError::DatabaseError(DatabaseErrorKind::ClosedConnection, _) => {
            UserPreferencesRepositoryError::connection("database connection error")
        }
        DieselError::DatabaseError(_, _) => UserPreferencesRepositoryError::query("database error"),
        _ => UserPreferencesRepositoryError::query("database error"),
    }
}

/// Convert a database row to a domain UserPreferences.
fn row_to_preferences(row: UserPreferencesRow) -> UserPreferences {
    let user_id = UserId::from_uuid(row.user_id);
    let unit_system = match row.unit_system.as_str() {
        "imperial" => UnitSystem::Imperial,
        "metric" => UnitSystem::Metric,
        other => {
            tracing::warn!(
                value = other,
                user_id = %row.user_id,
                "unrecognised unit_system value, defaulting to Metric"
            );
            UnitSystem::Metric
        }
    };

    UserPreferences {
        user_id,
        interest_theme_ids: row.interest_theme_ids,
        safety_toggle_ids: row.safety_toggle_ids,
        unit_system,
        #[expect(
            clippy::cast_sign_loss,
            reason = "revision is always non-negative in database"
        )]
        revision: row.revision as u32,
        updated_at: row.updated_at,
    }
}

/// Handle failed preferences update by checking if it's a revision mismatch or missing record.
async fn handle_preferences_update_failure<C>(
    conn: &mut C,
    user_id: uuid::Uuid,
    expected_revision: u32,
) -> UserPreferencesRepositoryError
where
    C: diesel_async::AsyncConnection<Backend = diesel::pg::Pg> + Send,
{
    let current_result = user_preferences::table
        .filter(user_preferences::user_id.eq(user_id))
        .select(UserPreferencesRow::as_select())
        .first(conn)
        .await
        .optional()
        .map_err(map_diesel_error);

    match current_result {
        Ok(Some(row)) => {
            #[expect(
                clippy::cast_sign_loss,
                reason = "revision is always non-negative in database"
            )]
            let actual = row.revision as u32;
            UserPreferencesRepositoryError::revision_mismatch(expected_revision, actual)
        }
        Ok(None) => UserPreferencesRepositoryError::query("preferences not found for update"),
        Err(e) => e,
    }
}

/// Cast domain revision (u32) to database revision (i32).
#[expect(
    clippy::cast_possible_wrap,
    reason = "revision values are always small positive integers"
)]
fn cast_revision_for_db(revision: u32) -> i32 {
    revision as i32
}

#[async_trait]
impl UserPreferencesRepository for DieselUserPreferencesRepository {
    async fn find_by_user_id(
        &self,
        user_id: &UserId,
    ) -> Result<Option<UserPreferences>, UserPreferencesRepositoryError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let result: Option<UserPreferencesRow> = user_preferences::table
            .filter(user_preferences::user_id.eq(user_id.as_uuid()))
            .select(UserPreferencesRow::as_select())
            .first(&mut conn)
            .await
            .optional()
            .map_err(map_diesel_error)?;

        Ok(result.map(row_to_preferences))
    }

    async fn save(
        &self,
        preferences: &UserPreferences,
        expected_revision: Option<u32>,
    ) -> Result<(), UserPreferencesRepositoryError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let unit_system_str = match preferences.unit_system {
            UnitSystem::Metric => "metric",
            UnitSystem::Imperial => "imperial",
        };

        let revision_i32 = cast_revision_for_db(preferences.revision);

        match expected_revision {
            None => {
                // Insert new preferences
                let new_row = NewUserPreferencesRow {
                    user_id: *preferences.user_id.as_uuid(),
                    interest_theme_ids: &preferences.interest_theme_ids,
                    safety_toggle_ids: &preferences.safety_toggle_ids,
                    unit_system: unit_system_str,
                    revision: revision_i32,
                };

                diesel::insert_into(user_preferences::table)
                    .values(&new_row)
                    .execute(&mut conn)
                    .await
                    .map(|_| ())
                    .map_err(map_diesel_error)
            }
            Some(expected) => {
                // Update with revision check
                let expected_i32 = cast_revision_for_db(expected);

                let update = UserPreferencesUpdate {
                    interest_theme_ids: &preferences.interest_theme_ids,
                    safety_toggle_ids: &preferences.safety_toggle_ids,
                    unit_system: unit_system_str,
                    revision: revision_i32,
                };

                let updated_rows = diesel::update(user_preferences::table)
                    .filter(
                        user_preferences::user_id
                            .eq(preferences.user_id.as_uuid())
                            .and(user_preferences::revision.eq(expected_i32)),
                    )
                    .set(&update)
                    .execute(&mut conn)
                    .await
                    .map_err(map_diesel_error)?;

                if updated_rows == 0 {
                    return Err(handle_preferences_update_failure(
                        &mut conn,
                        *preferences.user_id.as_uuid(),
                        expected,
                    )
                    .await);
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Regression coverage for this module.
    use super::*;
    use rstest::rstest;

    #[rstest]
    fn pool_error_maps_to_connection_error() {
        let pool_err = PoolError::checkout("connection refused");
        let repo_err = map_pool_error(pool_err);

        assert!(matches!(
            repo_err,
            UserPreferencesRepositoryError::Connection { .. }
        ));
        assert!(repo_err.to_string().contains("connection refused"));
    }

    #[rstest]
    fn diesel_error_maps_to_query_error() {
        let diesel_err = diesel::result::Error::NotFound;
        let repo_err = map_diesel_error(diesel_err);

        assert!(matches!(
            repo_err,
            UserPreferencesRepositoryError::Query { .. }
        ));
        assert!(repo_err.to_string().contains("record not found"));
    }

    #[rstest]
    fn row_to_preferences_converts_metric() {
        use chrono::Utc;

        let row = UserPreferencesRow {
            user_id: uuid::Uuid::new_v4(),
            interest_theme_ids: vec![uuid::Uuid::new_v4()],
            safety_toggle_ids: vec![],
            unit_system: "metric".to_string(),
            revision: 3,
            updated_at: Utc::now(),
        };

        let prefs = row_to_preferences(row);

        assert_eq!(prefs.unit_system, UnitSystem::Metric);
        assert_eq!(prefs.revision, 3);
        assert_eq!(prefs.interest_theme_ids.len(), 1);
    }

    #[rstest]
    fn row_to_preferences_converts_imperial() {
        use chrono::Utc;

        let row = UserPreferencesRow {
            user_id: uuid::Uuid::new_v4(),
            interest_theme_ids: vec![],
            safety_toggle_ids: vec![uuid::Uuid::new_v4(), uuid::Uuid::new_v4()],
            unit_system: "imperial".to_string(),
            revision: 1,
            updated_at: Utc::now(),
        };

        let prefs = row_to_preferences(row);

        assert_eq!(prefs.unit_system, UnitSystem::Imperial);
        assert_eq!(prefs.safety_toggle_ids.len(), 2);
    }
}
