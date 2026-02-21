//! PostgreSQL-backed `WalkSessionRepository` implementation using Diesel ORM.
//!
//! This adapter persists walk sessions and loads completion summaries through
//! validated domain constructors.

use async_trait::async_trait;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::domain::ports::{WalkSessionRepository, WalkSessionRepositoryError};
use crate::domain::{
    UserId, WalkCompletionSummary, WalkPrimaryStat, WalkPrimaryStatDraft, WalkSecondaryStat,
    WalkSecondaryStatDraft, WalkSession, WalkSessionDraft,
};

use super::diesel_basic_error_mapping::{map_basic_diesel_error, map_basic_pool_error};
use super::models::{NewWalkSessionRow, WalkSessionRow, WalkSessionUpdate};
use super::pool::{DbPool, PoolError};
use super::schema::walk_sessions;

/// Diesel-backed implementation of the walk session repository port.
#[derive(Clone)]
pub struct DieselWalkSessionRepository {
    pool: DbPool,
}

impl DieselWalkSessionRepository {
    /// Create a new repository with the given connection pool.
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

/// Map pool errors to domain repository errors.
fn map_pool_error(error: PoolError) -> WalkSessionRepositoryError {
    map_basic_pool_error(error, |message| {
        WalkSessionRepositoryError::connection(message)
    })
}

/// Map Diesel errors to domain repository errors.
fn map_diesel_error(error: diesel::result::Error) -> WalkSessionRepositoryError {
    map_basic_diesel_error(
        error,
        WalkSessionRepositoryError::query,
        WalkSessionRepositoryError::connection,
    )
}

fn serialize_primary_stats(
    session: &WalkSession,
) -> Result<serde_json::Value, WalkSessionRepositoryError> {
    serde_json::to_value(session.primary_stats())
        .map_err(|err| WalkSessionRepositoryError::query(format!("serialise primary stats: {err}")))
}

fn serialize_secondary_stats(
    session: &WalkSession,
) -> Result<serde_json::Value, WalkSessionRepositoryError> {
    serde_json::to_value(session.secondary_stats()).map_err(|err| {
        WalkSessionRepositoryError::query(format!("serialise secondary stats: {err}"))
    })
}

fn decode_stats<Draft, Domain, E>(
    stats: serde_json::Value,
    field_name: &str,
) -> Result<Vec<Domain>, WalkSessionRepositoryError>
where
    Draft: serde::de::DeserializeOwned,
    Domain: TryFrom<Draft, Error = E>,
    E: std::fmt::Display,
{
    let drafts: Vec<Draft> = serde_json::from_value(stats)
        .map_err(|err| WalkSessionRepositoryError::query(format!("decode {field_name}: {err}")))?;

    drafts
        .into_iter()
        .map(Domain::try_from)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| WalkSessionRepositoryError::query(err.to_string()))
}

fn decode_primary_stats(
    primary_stats: serde_json::Value,
) -> Result<Vec<WalkPrimaryStat>, WalkSessionRepositoryError> {
    decode_stats::<WalkPrimaryStatDraft, WalkPrimaryStat, _>(primary_stats, "primary_stats")
}

fn decode_secondary_stats(
    secondary_stats: serde_json::Value,
) -> Result<Vec<WalkSecondaryStat>, WalkSessionRepositoryError> {
    decode_stats::<WalkSecondaryStatDraft, WalkSecondaryStat, _>(secondary_stats, "secondary_stats")
}

/// Convert a database row into a validated domain walk session.
fn row_to_walk_session(row: WalkSessionRow) -> Result<WalkSession, WalkSessionRepositoryError> {
    let WalkSessionRow {
        id,
        user_id,
        route_id,
        started_at,
        ended_at,
        primary_stats,
        secondary_stats,
        highlighted_poi_ids,
        created_at: _,
        updated_at: _,
    } = row;

    let primary_stats = decode_primary_stats(primary_stats)?;
    let secondary_stats = decode_secondary_stats(secondary_stats)?;

    WalkSession::new(WalkSessionDraft {
        id,
        user_id: UserId::from_uuid(user_id),
        route_id,
        started_at,
        ended_at,
        primary_stats,
        secondary_stats,
        highlighted_poi_ids,
    })
    .map_err(|err| WalkSessionRepositoryError::query(err.to_string()))
}

#[async_trait]
impl WalkSessionRepository for DieselWalkSessionRepository {
    async fn save(&self, session: &WalkSession) -> Result<(), WalkSessionRepositoryError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;
        let primary_stats = serialize_primary_stats(session)?;
        let secondary_stats = serialize_secondary_stats(session)?;

        let new_row = NewWalkSessionRow {
            id: session.id(),
            user_id: *session.user_id().as_uuid(),
            route_id: session.route_id(),
            started_at: session.started_at(),
            ended_at: session.ended_at(),
            primary_stats: &primary_stats,
            secondary_stats: &secondary_stats,
            highlighted_poi_ids: session.highlighted_poi_ids(),
        };

        let update_row = WalkSessionUpdate {
            user_id: *session.user_id().as_uuid(),
            route_id: session.route_id(),
            started_at: session.started_at(),
            ended_at: session.ended_at(),
            primary_stats: &primary_stats,
            secondary_stats: &secondary_stats,
            highlighted_poi_ids: session.highlighted_poi_ids(),
        };

        diesel::insert_into(walk_sessions::table)
            .values(&new_row)
            .on_conflict(walk_sessions::id)
            .do_update()
            .set(&update_row)
            .execute(&mut conn)
            .await
            .map(|_| ())
            .map_err(map_diesel_error)
    }

    async fn find_by_id(
        &self,
        session_id: &Uuid,
    ) -> Result<Option<WalkSession>, WalkSessionRepositoryError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let row = walk_sessions::table
            .filter(walk_sessions::id.eq(session_id))
            .select(WalkSessionRow::as_select())
            .first::<WalkSessionRow>(&mut conn)
            .await
            .optional()
            .map_err(map_diesel_error)?;

        row.map(row_to_walk_session).transpose()
    }

    async fn list_completion_summaries_for_user(
        &self,
        user_id: &UserId,
    ) -> Result<Vec<WalkCompletionSummary>, WalkSessionRepositoryError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let rows: Vec<WalkSessionRow> = walk_sessions::table
            .filter(
                walk_sessions::user_id
                    .eq(user_id.as_uuid())
                    .and(walk_sessions::ended_at.is_not_null()),
            )
            .order((walk_sessions::ended_at.desc(), walk_sessions::id.desc()))
            .select(WalkSessionRow::as_select())
            .load(&mut conn)
            .await
            .map_err(map_diesel_error)?;

        rows.into_iter()
            .map(|row| {
                row_to_walk_session(row)?
                    .completion_summary()
                    .map_err(|err| WalkSessionRepositoryError::query(err.to_string()))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    //! Regression coverage for error mapping and row conversion edge cases.

    use chrono::{Duration, Utc};
    use rstest::{fixture, rstest};
    use serde_json::json;

    use super::*;

    #[fixture]
    fn valid_row() -> WalkSessionRow {
        let started_at = Utc::now();
        WalkSessionRow {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            route_id: Uuid::new_v4(),
            started_at,
            ended_at: Some(started_at + Duration::minutes(10)),
            primary_stats: json!([
                { "kind": "distance", "value": 1000.0 },
                { "kind": "duration", "value": 600.0 }
            ]),
            secondary_stats: json!([
                { "kind": "energy", "value": 120.0, "unit": "kcal" }
            ]),
            highlighted_poi_ids: vec![Uuid::new_v4()],
            created_at: started_at,
            updated_at: started_at,
        }
    }

    #[rstest]
    fn pool_error_maps_to_connection_error() {
        let pool_err = PoolError::checkout("connection refused");
        let repo_err = map_pool_error(pool_err);

        assert!(matches!(
            repo_err,
            WalkSessionRepositoryError::Connection { .. }
        ));
        assert!(repo_err.to_string().contains("connection refused"));
    }

    #[rstest]
    fn diesel_error_maps_to_query_error() {
        let diesel_err = diesel::result::Error::NotFound;
        let repo_err = map_diesel_error(diesel_err);

        assert!(matches!(repo_err, WalkSessionRepositoryError::Query { .. }));
        assert!(repo_err.to_string().contains("record not found"));
    }

    #[rstest]
    fn row_conversion_rejects_invalid_primary_stats_json(mut valid_row: WalkSessionRow) {
        valid_row.primary_stats = json!({ "not": "an-array" });

        let error = row_to_walk_session(valid_row).expect_err("invalid json should fail");
        assert!(matches!(error, WalkSessionRepositoryError::Query { .. }));
        assert!(error.to_string().contains("decode primary_stats"));
    }

    #[rstest]
    fn row_conversion_rejects_invalid_session_timestamps(mut valid_row: WalkSessionRow) {
        valid_row.ended_at = Some(valid_row.started_at - Duration::seconds(1));

        let error = row_to_walk_session(valid_row).expect_err("invalid timestamps should fail");
        assert!(matches!(error, WalkSessionRepositoryError::Query { .. }));
        assert!(error.to_string().contains("ended_at"));
    }
}
