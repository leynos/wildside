//! PostgreSQL-backed test repositories for offline bundles and walk sessions.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use backend::domain::ports::{
    OfflineBundleRepository, OfflineBundleRepositoryError, WalkSessionRepository,
    WalkSessionRepositoryError,
};
use backend::domain::{
    BoundingBox, OfflineBundle, OfflineBundleDraft, OfflineBundleKind, OfflineBundleStatus, UserId,
    WalkCompletionSummary, WalkPrimaryStat, WalkPrimaryStatDraft, WalkSecondaryStat,
    WalkSecondaryStatDraft, WalkSession, WalkSessionDraft, ZoomRange,
};
use chrono::{DateTime, Utc};
use postgres::{Client, Row};
use uuid::Uuid;

use crate::support::format_postgres_error;

#[derive(Clone)]
pub struct PgOfflineBundleRepository {
    client: Arc<Mutex<Client>>,
}

impl PgOfflineBundleRepository {
    pub fn new(client: Arc<Mutex<Client>>) -> Self {
        Self { client }
    }
}

#[derive(Clone)]
pub struct PgWalkSessionRepository {
    client: Arc<Mutex<Client>>,
}

impl PgWalkSessionRepository {
    pub fn new(client: Arc<Mutex<Client>>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl OfflineBundleRepository for PgOfflineBundleRepository {
    async fn find_by_id(
        &self,
        bundle_id: &Uuid,
    ) -> Result<Option<OfflineBundle>, OfflineBundleRepositoryError> {
        let mut guard = self.client.lock().expect("client lock");
        let row = guard
            .query_opt(
                "SELECT id, owner_user_id, device_id, kind, route_id, region_id, bounds,
                        min_zoom, max_zoom, estimated_size_bytes,
                        created_at::text AS created_at, updated_at::text AS updated_at,
                        status, progress
                 FROM offline_bundles
                 WHERE id = $1",
                &[bundle_id],
            )
            .map_err(|err| OfflineBundleRepositoryError::query(format_postgres_error(&err)))?;

        row.map(row_to_offline_bundle).transpose()
    }

    async fn list_for_owner_and_device(
        &self,
        owner_user_id: Option<UserId>,
        device_id: &str,
    ) -> Result<Vec<OfflineBundle>, OfflineBundleRepositoryError> {
        let mut guard = self.client.lock().expect("client lock");
        let rows = match owner_user_id {
            Some(user_id) => guard
                .query(
                    "SELECT id, owner_user_id, device_id, kind, route_id, region_id, bounds,
                            min_zoom, max_zoom, estimated_size_bytes,
                            created_at::text AS created_at, updated_at::text AS updated_at,
                            status, progress
                     FROM offline_bundles
                     WHERE owner_user_id = $1 AND device_id = $2
                     ORDER BY created_at ASC",
                    &[user_id.as_uuid(), &device_id],
                )
                .map_err(|err| OfflineBundleRepositoryError::query(format_postgres_error(&err)))?,
            None => guard
                .query(
                    "SELECT id, owner_user_id, device_id, kind, route_id, region_id, bounds,
                            min_zoom, max_zoom, estimated_size_bytes,
                            created_at::text AS created_at, updated_at::text AS updated_at,
                            status, progress
                     FROM offline_bundles
                     WHERE owner_user_id IS NULL AND device_id = $1
                     ORDER BY created_at ASC",
                    &[&device_id],
                )
                .map_err(|err| OfflineBundleRepositoryError::query(format_postgres_error(&err)))?,
        };

        rows.into_iter().map(row_to_offline_bundle).collect()
    }

    async fn save(&self, bundle: &OfflineBundle) -> Result<(), OfflineBundleRepositoryError> {
        let estimated_size = i64::try_from(bundle.estimated_size_bytes())
            .map_err(|_| OfflineBundleRepositoryError::query("estimated_size_bytes overflow"))?;
        let mut guard = self.client.lock().expect("client lock");
        let owner_user_id = bundle.owner_user_id().map(|id| *id.as_uuid());
        let route_id = bundle.route_id();
        let region_id = bundle.region_id().map(str::to_owned);
        let bounds = bundle.bounds().as_array().to_vec();
        let created_at = bundle.created_at().to_rfc3339();
        let updated_at = bundle.updated_at().to_rfc3339();

        guard
            .execute(
                "INSERT INTO offline_bundles
                    (id, owner_user_id, device_id, kind, route_id, region_id, bounds,
                     min_zoom, max_zoom, estimated_size_bytes, created_at, updated_at,
                     status, progress)
                 VALUES ($1, $2, $3, $4, $5, $6, $7,
                         $8, $9, $10, ($11::text)::timestamptz, ($12::text)::timestamptz,
                         $13, $14)
                 ON CONFLICT (id) DO UPDATE SET
                    owner_user_id = EXCLUDED.owner_user_id,
                    device_id = EXCLUDED.device_id,
                    kind = EXCLUDED.kind,
                    route_id = EXCLUDED.route_id,
                    region_id = EXCLUDED.region_id,
                    bounds = EXCLUDED.bounds,
                    min_zoom = EXCLUDED.min_zoom,
                    max_zoom = EXCLUDED.max_zoom,
                    estimated_size_bytes = EXCLUDED.estimated_size_bytes,
                    updated_at = EXCLUDED.updated_at,
                    status = EXCLUDED.status,
                    progress = EXCLUDED.progress",
                &[
                    &bundle.id(),
                    &owner_user_id,
                    &bundle.device_id(),
                    &bundle.kind().as_str(),
                    &route_id,
                    &region_id,
                    &bounds,
                    &(i32::from(bundle.zoom_range().min_zoom())),
                    &(i32::from(bundle.zoom_range().max_zoom())),
                    &estimated_size,
                    &created_at,
                    &updated_at,
                    &bundle.status().as_str(),
                    &bundle.progress(),
                ],
            )
            .map(|_| ())
            .map_err(|err| OfflineBundleRepositoryError::query(format_postgres_error(&err)))
    }

    async fn delete(&self, bundle_id: &Uuid) -> Result<bool, OfflineBundleRepositoryError> {
        let mut guard = self.client.lock().expect("client lock");
        guard
            .execute("DELETE FROM offline_bundles WHERE id = $1", &[bundle_id])
            .map(|count| count > 0)
            .map_err(|err| OfflineBundleRepositoryError::query(format_postgres_error(&err)))
    }
}

#[async_trait]
impl WalkSessionRepository for PgWalkSessionRepository {
    async fn save(&self, session: &WalkSession) -> Result<(), WalkSessionRepositoryError> {
        let primary_stats = serde_json::to_string(session.primary_stats()).map_err(|err| {
            WalkSessionRepositoryError::query(format!("serialise primary stats: {err}"))
        })?;
        let secondary_stats = serde_json::to_string(session.secondary_stats()).map_err(|err| {
            WalkSessionRepositoryError::query(format!("serialise secondary stats: {err}"))
        })?;
        let started_at = session.started_at().to_rfc3339();
        let ended_at = session.ended_at().map(|value| value.to_rfc3339());

        let mut guard = self.client.lock().expect("client lock");
        guard
            .execute(
                "INSERT INTO walk_sessions
                    (id, user_id, route_id, started_at, ended_at, primary_stats,
                     secondary_stats, highlighted_poi_ids)
                 VALUES ($1, $2, $3, ($4::text)::timestamptz,
                         ($5::text)::timestamptz, ($6::text)::jsonb,
                         ($7::text)::jsonb, $8)
                 ON CONFLICT (id) DO UPDATE SET
                    user_id = EXCLUDED.user_id,
                    route_id = EXCLUDED.route_id,
                    started_at = EXCLUDED.started_at,
                    ended_at = EXCLUDED.ended_at,
                    primary_stats = EXCLUDED.primary_stats,
                    secondary_stats = EXCLUDED.secondary_stats,
                    highlighted_poi_ids = EXCLUDED.highlighted_poi_ids",
                &[
                    &session.id(),
                    session.user_id().as_uuid(),
                    &session.route_id(),
                    &started_at,
                    &ended_at,
                    &primary_stats,
                    &secondary_stats,
                    &session.highlighted_poi_ids(),
                ],
            )
            .map(|_| ())
            .map_err(|err| WalkSessionRepositoryError::query(format_postgres_error(&err)))
    }

    async fn find_by_id(
        &self,
        session_id: &Uuid,
    ) -> Result<Option<WalkSession>, WalkSessionRepositoryError> {
        let mut guard = self.client.lock().expect("client lock");
        let row = guard
            .query_opt(
                "SELECT id, user_id, route_id, started_at::text AS started_at,
                        ended_at::text AS ended_at, primary_stats::text AS primary_stats,
                        secondary_stats::text AS secondary_stats, highlighted_poi_ids
                 FROM walk_sessions
                 WHERE id = $1",
                &[session_id],
            )
            .map_err(|err| WalkSessionRepositoryError::query(format_postgres_error(&err)))?;

        row.map(row_to_walk_session).transpose()
    }

    async fn list_completion_summaries_for_user(
        &self,
        user_id: &UserId,
    ) -> Result<Vec<WalkCompletionSummary>, WalkSessionRepositoryError> {
        let mut guard = self.client.lock().expect("client lock");
        let rows = guard
            .query(
                "SELECT id, user_id, route_id, started_at::text AS started_at,
                        ended_at::text AS ended_at, primary_stats::text AS primary_stats,
                        secondary_stats::text AS secondary_stats, highlighted_poi_ids
                 FROM walk_sessions
                 WHERE user_id = $1 AND ended_at IS NOT NULL
                 ORDER BY ended_at DESC",
                &[user_id.as_uuid()],
            )
            .map_err(|err| WalkSessionRepositoryError::query(format_postgres_error(&err)))?;

        rows.into_iter()
            .map(|row| {
                row_to_walk_session(row)?
                    .completion_summary()
                    .map_err(|err| WalkSessionRepositoryError::query(err.to_string()))
            })
            .collect()
    }
}

pub fn create_contract_tables(client: &mut Client) -> Result<(), postgres::Error> {
    client.batch_execute(
        "CREATE TABLE IF NOT EXISTS offline_bundles (
            id UUID PRIMARY KEY,
            owner_user_id UUID NULL,
            device_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            route_id UUID NULL,
            region_id TEXT NULL,
            bounds DOUBLE PRECISION[] NOT NULL,
            min_zoom INTEGER NOT NULL,
            max_zoom INTEGER NOT NULL,
            estimated_size_bytes BIGINT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL,
            updated_at TIMESTAMPTZ NOT NULL,
            status TEXT NOT NULL,
            progress REAL NOT NULL
        );

        CREATE TABLE IF NOT EXISTS walk_sessions (
            id UUID PRIMARY KEY,
            user_id UUID NOT NULL,
            route_id UUID NOT NULL,
            started_at TIMESTAMPTZ NOT NULL,
            ended_at TIMESTAMPTZ NULL,
            primary_stats JSONB NOT NULL,
            secondary_stats JSONB NOT NULL,
            highlighted_poi_ids UUID[] NOT NULL
        );",
    )
}

pub fn drop_table(database_url: &str, table_name: &str) -> Result<(), postgres::Error> {
    let mut client = Client::connect(database_url, postgres::NoTls)?;
    let escaped_table_name = table_name.replace('"', "\"\"");
    let statement = format!(r#"DROP TABLE IF EXISTS "{escaped_table_name}""#);
    client.batch_execute(statement.as_str())
}

fn row_to_offline_bundle(row: Row) -> Result<OfflineBundle, OfflineBundleRepositoryError> {
    let bounds: Vec<f64> = row.get("bounds");
    if bounds.len() != 4 {
        return Err(OfflineBundleRepositoryError::query(format!(
            "bounds expected 4 values, found {}",
            bounds.len()
        )));
    }

    let kind: String = row.get("kind");
    let status: String = row.get("status");
    let min_zoom_i32: i32 = row.get("min_zoom");
    let max_zoom_i32: i32 = row.get("max_zoom");

    let min_zoom = u8::try_from(min_zoom_i32)
        .map_err(|_| OfflineBundleRepositoryError::query("min_zoom out of range for u8"))?;
    let max_zoom = u8::try_from(max_zoom_i32)
        .map_err(|_| OfflineBundleRepositoryError::query("max_zoom out of range for u8"))?;

    let estimated_size: i64 = row.get("estimated_size_bytes");
    let estimated_size_bytes = u64::try_from(estimated_size)
        .map_err(|_| OfflineBundleRepositoryError::query("estimated_size_bytes is negative"))?;

    OfflineBundle::new(OfflineBundleDraft {
        id: row.get("id"),
        owner_user_id: row
            .get::<_, Option<Uuid>>("owner_user_id")
            .map(UserId::from_uuid),
        device_id: row.get("device_id"),
        kind: kind
            .parse::<OfflineBundleKind>()
            .map_err(|err| OfflineBundleRepositoryError::query(err.to_string()))?,
        route_id: row.get("route_id"),
        region_id: row.get("region_id"),
        bounds: BoundingBox::new(bounds[0], bounds[1], bounds[2], bounds[3])
            .map_err(|err| OfflineBundleRepositoryError::query(err.to_string()))?,
        zoom_range: ZoomRange::new(min_zoom, max_zoom)
            .map_err(|err| OfflineBundleRepositoryError::query(err.to_string()))?,
        estimated_size_bytes,
        created_at: parse_timestamptz(row.get("created_at"))
            .map_err(OfflineBundleRepositoryError::query)?,
        updated_at: parse_timestamptz(row.get("updated_at"))
            .map_err(OfflineBundleRepositoryError::query)?,
        status: status
            .parse::<OfflineBundleStatus>()
            .map_err(|err| OfflineBundleRepositoryError::query(err.to_string()))?,
        progress: row.get("progress"),
    })
    .map_err(|err| OfflineBundleRepositoryError::query(err.to_string()))
}

fn row_to_walk_session(row: Row) -> Result<WalkSession, WalkSessionRepositoryError> {
    let primary_stats_draft: Vec<WalkPrimaryStatDraft> =
        serde_json::from_str(row.get::<_, String>("primary_stats").as_str())
            .map_err(|err| WalkSessionRepositoryError::query(err.to_string()))?;
    let secondary_stats_draft: Vec<WalkSecondaryStatDraft> =
        serde_json::from_str(row.get::<_, String>("secondary_stats").as_str())
            .map_err(|err| WalkSessionRepositoryError::query(err.to_string()))?;

    let primary_stats = primary_stats_draft
        .into_iter()
        .map(WalkPrimaryStat::try_from)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| WalkSessionRepositoryError::query(err.to_string()))?;
    let secondary_stats = secondary_stats_draft
        .into_iter()
        .map(WalkSecondaryStat::try_from)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| WalkSessionRepositoryError::query(err.to_string()))?;

    WalkSession::new(WalkSessionDraft {
        id: row.get("id"),
        user_id: UserId::from_uuid(row.get("user_id")),
        route_id: row.get("route_id"),
        started_at: parse_timestamptz(row.get("started_at"))
            .map_err(WalkSessionRepositoryError::query)?,
        ended_at: row
            .get::<_, Option<String>>("ended_at")
            .map(parse_timestamptz)
            .transpose()
            .map_err(WalkSessionRepositoryError::query)?,
        primary_stats,
        secondary_stats,
        highlighted_poi_ids: row.get("highlighted_poi_ids"),
    })
    .map_err(|err| WalkSessionRepositoryError::query(err.to_string()))
}

fn parse_timestamptz(value: String) -> Result<DateTime<Utc>, String> {
    DateTime::parse_from_rfc3339(value.as_str())
        .or_else(|_| DateTime::parse_from_str(value.as_str(), "%Y-%m-%d %H:%M:%S%.f%#z"))
        .or_else(|_| DateTime::parse_from_str(value.as_str(), "%Y-%m-%d %H:%M:%S%.f%:z"))
        .or_else(|_| DateTime::parse_from_str(value.as_str(), "%Y-%m-%d %H:%M:%S%.f%z"))
        .map(|parsed| parsed.with_timezone(&Utc))
        .map_err(|err| format!("invalid timestamptz '{value}': {err}"))
}
