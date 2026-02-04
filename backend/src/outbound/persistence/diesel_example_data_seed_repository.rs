//! PostgreSQL-backed example data seeding adapter.
//!
//! This adapter implements the `ExampleDataSeedRepository` port, applying
//! example data within a single transaction. It records the seed run and
//! inserts or updates users and their preferences atomically.

use async_trait::async_trait;
use diesel::prelude::*;
use diesel::upsert::excluded;
use diesel_async::scoped_futures::ScopedFutureExt;
use diesel_async::{AsyncConnection, RunQueryDsl};
use tracing::debug;

use crate::domain::ports::{
    ExampleDataSeedRepository, ExampleDataSeedRepositoryError, ExampleDataSeedRequest,
    ExampleDataSeedUser, SeedingResult,
};

use super::models::{NewExampleDataRunRow, NewUserPreferencesRow, NewUserRow};
use super::pool::{DbPool, PoolError};
use super::schema::{example_data_runs, user_preferences, users};

/// Diesel-backed implementation of the example data seeding repository.
#[derive(Clone)]
pub struct DieselExampleDataSeedRepository {
    pool: DbPool,
}

impl DieselExampleDataSeedRepository {
    /// Create a new seeding repository with the given connection pool.
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use backend::outbound::persistence::{
    ///     DbPool, DieselExampleDataSeedRepository, PoolConfig,
    /// };
    ///
    /// # async fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let pool = DbPool::new(PoolConfig::new("postgres://localhost")).await?;
    /// let repository = DieselExampleDataSeedRepository::new(pool);
    /// # let _ = repository;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

/// Map pool errors to domain persistence errors.
fn map_pool_error(error: PoolError) -> ExampleDataSeedRepositoryError {
    match error {
        PoolError::Checkout { message } | PoolError::Build { message } => {
            ExampleDataSeedRepositoryError::connection(message)
        }
    }
}

/// Map Diesel errors to domain persistence errors.
fn map_diesel_error(error: diesel::result::Error) -> ExampleDataSeedRepositoryError {
    use diesel::result::{DatabaseErrorKind, Error as DieselError};

    let error_message = error.to_string();
    match &error {
        DieselError::DatabaseError(kind, info) => {
            debug!(
                ?kind,
                message = info.message(),
                error = %error_message,
                "diesel operation failed"
            );
        }
        _ => debug!(
            error_type = %std::any::type_name_of_val(&error),
            error = %error_message,
            "diesel operation failed"
        ),
    }

    match error {
        DieselError::NotFound => ExampleDataSeedRepositoryError::query("record not found"),
        DieselError::DatabaseError(DatabaseErrorKind::ClosedConnection, info) => {
            ExampleDataSeedRepositoryError::connection(info.message().to_owned())
        }
        DieselError::DatabaseError(_, info) => {
            ExampleDataSeedRepositoryError::query(info.message().to_owned())
        }
        _ => ExampleDataSeedRepositoryError::query(error_message),
    }
}

fn map_seed_users(
    users: &[ExampleDataSeedUser],
) -> Result<(Vec<NewUserRow<'_>>, Vec<NewUserPreferencesRow<'_>>), ExampleDataSeedRepositoryError> {
    let mut user_rows = Vec::with_capacity(users.len());
    let mut preference_rows = Vec::with_capacity(users.len());

    for seed_user in users {
        let user = &seed_user.user;
        let preferences = &seed_user.preferences;

        user_rows.push(NewUserRow {
            id: *user.id().as_uuid(),
            display_name: user.display_name().as_ref(),
        });

        let revision = i32::try_from(preferences.revision)
            .map_err(|_| ExampleDataSeedRepositoryError::query("preferences revision overflow"))?;

        preference_rows.push(NewUserPreferencesRow {
            user_id: *preferences.user_id.as_uuid(),
            interest_theme_ids: &preferences.interest_theme_ids,
            safety_toggle_ids: &preferences.safety_toggle_ids,
            unit_system: preferences.unit_system.as_str(),
            revision,
        });
    }

    Ok((user_rows, preference_rows))
}

#[async_trait]
impl ExampleDataSeedRepository for DieselExampleDataSeedRepository {
    async fn seed_example_data(
        &self,
        request: ExampleDataSeedRequest,
    ) -> Result<SeedingResult, ExampleDataSeedRepositoryError> {
        let ExampleDataSeedRequest {
            seed_key,
            user_count,
            seed,
            users,
        } = request;
        let (user_rows, preference_rows) = map_seed_users(&users)?;
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let result = conn
            .transaction(|conn| {
                async move {
                    let new_run = NewExampleDataRunRow {
                        seed_key: seed_key.as_str(),
                        user_count,
                        seed,
                    };

                    let rows_affected = diesel::insert_into(example_data_runs::table)
                        .values(&new_run)
                        .on_conflict(example_data_runs::seed_key)
                        .do_nothing()
                        .execute(conn)
                        .await?;

                    if rows_affected == 0 {
                        return Ok(SeedingResult::AlreadySeeded);
                    }

                    if user_rows.is_empty() {
                        return Ok(SeedingResult::Applied);
                    }

                    diesel::insert_into(users::table)
                        .values(&user_rows)
                        .on_conflict(users::id)
                        .do_update()
                        .set(users::display_name.eq(excluded(users::display_name)))
                        .execute(conn)
                        .await?;

                    diesel::insert_into(user_preferences::table)
                        .values(&preference_rows)
                        .on_conflict(user_preferences::user_id)
                        .do_update()
                        .set((
                            user_preferences::interest_theme_ids
                                .eq(excluded(user_preferences::interest_theme_ids)),
                            user_preferences::safety_toggle_ids
                                .eq(excluded(user_preferences::safety_toggle_ids)),
                            user_preferences::unit_system
                                .eq(excluded(user_preferences::unit_system)),
                            user_preferences::revision.eq(excluded(user_preferences::revision)),
                        ))
                        .execute(conn)
                        .await?;

                    Ok(SeedingResult::Applied)
                }
                .scope_boxed()
            })
            .await
            .map_err(map_diesel_error)?;

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    //! Regression coverage for seed repository error mapping.
    use super::*;
    use rstest::rstest;

    #[rstest]
    fn pool_error_maps_to_connection_error() {
        let pool_err = PoolError::checkout("connection refused");
        let persistence_err = map_pool_error(pool_err);

        assert!(matches!(
            persistence_err,
            ExampleDataSeedRepositoryError::Connection { .. }
        ));
        assert!(
            persistence_err.to_string().contains("connection refused"),
            "preserve useful diagnostics"
        );
    }

    #[rstest]
    fn diesel_error_maps_to_query_error() {
        let diesel_err = diesel::result::Error::NotFound;
        let persistence_err = map_diesel_error(diesel_err);

        assert!(matches!(
            persistence_err,
            ExampleDataSeedRepositoryError::Query { .. }
        ));
        assert!(
            persistence_err.to_string().contains("record not found"),
            "preserve stable, user-facing diagnostics"
        );
    }
}
