//! PostgreSQL-backed `OfflineBundleRepository` implementation using Diesel ORM.
//!
//! This adapter persists offline bundle manifests and translates row payloads
//! into validated domain entities.

use async_trait::async_trait;
use diesel::prelude::*;
use diesel_async::RunQueryDsl;
use uuid::Uuid;

use crate::domain::ports::{OfflineBundleRepository, OfflineBundleRepositoryError};
use crate::domain::{
    BoundingBox, OfflineBundle, OfflineBundleDraft, OfflineBundleKind, OfflineBundleStatus, UserId,
    ZoomRange,
};

use super::diesel_basic_error_mapping::{map_basic_diesel_error, map_basic_pool_error};
use super::models::{NewOfflineBundleRow, OfflineBundleRow, OfflineBundleUpdate};
use super::pool::{DbPool, PoolError};
use super::schema::offline_bundles;

/// Diesel-backed implementation of the offline bundle repository port.
#[derive(Clone)]
pub struct DieselOfflineBundleRepository {
    pool: DbPool,
}

impl DieselOfflineBundleRepository {
    /// Create a new repository with the given connection pool.
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }
}

/// Map pool errors to domain repository errors.
fn map_pool_error(error: PoolError) -> OfflineBundleRepositoryError {
    map_basic_pool_error(error, |message| {
        OfflineBundleRepositoryError::connection(message)
    })
}

/// Map Diesel errors to domain repository errors.
fn map_diesel_error(error: diesel::result::Error) -> OfflineBundleRepositoryError {
    map_basic_diesel_error(
        error,
        OfflineBundleRepositoryError::query,
        OfflineBundleRepositoryError::connection,
    )
}

/// Convert a database row into a validated domain offline bundle.
fn row_to_offline_bundle(
    row: OfflineBundleRow,
) -> Result<OfflineBundle, OfflineBundleRepositoryError> {
    let OfflineBundleRow {
        id,
        owner_user_id,
        device_id,
        kind,
        route_id,
        region_id,
        bounds,
        min_zoom,
        max_zoom,
        estimated_size_bytes,
        created_at,
        updated_at,
        status,
        progress,
    } = row;

    let bounds_values: [f64; 4] = bounds
        .try_into()
        .map_err(|_| OfflineBundleRepositoryError::query("bounds expected 4 values"))?;

    let min_zoom = u8::try_from(min_zoom)
        .map_err(|_| OfflineBundleRepositoryError::query("min_zoom out of range for u8"))?;
    let max_zoom = u8::try_from(max_zoom)
        .map_err(|_| OfflineBundleRepositoryError::query("max_zoom out of range for u8"))?;
    let estimated_size_bytes = u64::try_from(estimated_size_bytes)
        .map_err(|_| OfflineBundleRepositoryError::query("estimated_size_bytes is negative"))?;

    let kind = kind
        .parse::<OfflineBundleKind>()
        .map_err(|err| OfflineBundleRepositoryError::query(err.to_string()))?;
    let status = status
        .parse::<OfflineBundleStatus>()
        .map_err(|err| OfflineBundleRepositoryError::query(err.to_string()))?;

    OfflineBundle::new(OfflineBundleDraft {
        id,
        owner_user_id: owner_user_id.map(UserId::from_uuid),
        device_id,
        kind,
        route_id,
        region_id,
        bounds: BoundingBox::new(
            bounds_values[0],
            bounds_values[1],
            bounds_values[2],
            bounds_values[3],
        )
        .map_err(|err| OfflineBundleRepositoryError::query(err.to_string()))?,
        zoom_range: ZoomRange::new(min_zoom, max_zoom)
            .map_err(|err| OfflineBundleRepositoryError::query(err.to_string()))?,
        estimated_size_bytes,
        created_at,
        updated_at,
        status,
        progress,
    })
    .map_err(|err| OfflineBundleRepositoryError::query(err.to_string()))
}

/// Shared row payload for insert and update statements.
struct OfflineBundleRowPayload<'a> {
    id: Uuid,
    owner_user_id: Option<Uuid>,
    device_id: &'a str,
    kind: &'a str,
    route_id: Option<Uuid>,
    region_id: Option<&'a str>,
    bounds: [f64; 4],
    min_zoom: i32,
    max_zoom: i32,
    estimated_size_bytes: i64,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
    status: &'a str,
    progress: f32,
}

impl<'a> OfflineBundleRowPayload<'a> {
    fn from_bundle(bundle: &'a OfflineBundle) -> Result<Self, OfflineBundleRepositoryError> {
        let estimated_size_bytes = i64::try_from(bundle.estimated_size_bytes())
            .map_err(|_| OfflineBundleRepositoryError::query("estimated_size_bytes overflow"))?;

        Ok(Self {
            id: bundle.id(),
            owner_user_id: bundle.owner_user_id().map(|value| *value.as_uuid()),
            device_id: bundle.device_id(),
            kind: bundle.kind().as_str(),
            route_id: bundle.route_id(),
            region_id: bundle.region_id(),
            bounds: bundle.bounds().as_array(),
            min_zoom: i32::from(bundle.zoom_range().min_zoom()),
            max_zoom: i32::from(bundle.zoom_range().max_zoom()),
            estimated_size_bytes,
            created_at: bundle.created_at(),
            updated_at: bundle.updated_at(),
            status: bundle.status().as_str(),
            progress: bundle.progress(),
        })
    }

    fn to_new_row(&'a self) -> NewOfflineBundleRow<'a> {
        NewOfflineBundleRow {
            id: self.id,
            owner_user_id: self.owner_user_id,
            device_id: self.device_id,
            kind: self.kind,
            route_id: self.route_id,
            region_id: self.region_id,
            bounds: self.bounds.as_slice(),
            min_zoom: self.min_zoom,
            max_zoom: self.max_zoom,
            estimated_size_bytes: self.estimated_size_bytes,
            created_at: self.created_at,
            updated_at: self.updated_at,
            status: self.status,
            progress: self.progress,
        }
    }

    fn to_update_row(&'a self) -> OfflineBundleUpdate<'a> {
        OfflineBundleUpdate {
            owner_user_id: self.owner_user_id,
            device_id: self.device_id,
            kind: self.kind,
            route_id: self.route_id,
            region_id: self.region_id,
            bounds: self.bounds.as_slice(),
            min_zoom: self.min_zoom,
            max_zoom: self.max_zoom,
            estimated_size_bytes: self.estimated_size_bytes,
            updated_at: self.updated_at,
            status: self.status,
            progress: self.progress,
        }
    }
}

#[async_trait]
impl OfflineBundleRepository for DieselOfflineBundleRepository {
    async fn find_by_id(
        &self,
        bundle_id: &Uuid,
    ) -> Result<Option<OfflineBundle>, OfflineBundleRepositoryError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let row = offline_bundles::table
            .filter(offline_bundles::id.eq(bundle_id))
            .select(OfflineBundleRow::as_select())
            .first::<OfflineBundleRow>(&mut conn)
            .await
            .optional()
            .map_err(map_diesel_error)?;

        row.map(row_to_offline_bundle).transpose()
    }

    async fn list_for_owner_and_device(
        &self,
        owner_user_id: Option<UserId>,
        device_id: &str,
    ) -> Result<Vec<OfflineBundle>, OfflineBundleRepositoryError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let mut query = offline_bundles::table
            .filter(offline_bundles::device_id.eq(device_id))
            .into_boxed();

        if let Some(owner_user_id) = owner_user_id {
            let owner_uuid = *owner_user_id.as_uuid();
            query = query.filter(offline_bundles::owner_user_id.eq(owner_uuid));
        } else {
            query = query.filter(offline_bundles::owner_user_id.is_null());
        }

        let rows: Vec<OfflineBundleRow> = query
            .order((offline_bundles::created_at.asc(), offline_bundles::id.asc()))
            .select(OfflineBundleRow::as_select())
            .load(&mut conn)
            .await
            .map_err(map_diesel_error)?;

        rows.into_iter().map(row_to_offline_bundle).collect()
    }

    async fn save(&self, bundle: &OfflineBundle) -> Result<(), OfflineBundleRepositoryError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;
        let payload = OfflineBundleRowPayload::from_bundle(bundle)?;
        let new_row = payload.to_new_row();
        let update_row = payload.to_update_row();

        diesel::insert_into(offline_bundles::table)
            .values(&new_row)
            .on_conflict(offline_bundles::id)
            .do_update()
            .set(&update_row)
            .execute(&mut conn)
            .await
            .map(|_| ())
            .map_err(map_diesel_error)
    }

    async fn delete(&self, bundle_id: &Uuid) -> Result<bool, OfflineBundleRepositoryError> {
        let mut conn = self.pool.get().await.map_err(map_pool_error)?;

        let deleted =
            diesel::delete(offline_bundles::table.filter(offline_bundles::id.eq(bundle_id)))
                .execute(&mut conn)
                .await
                .map_err(map_diesel_error)?;

        Ok(deleted > 0)
    }
}

#[cfg(test)]
mod tests {
    //! Regression coverage for error mapping and row conversion edge cases.

    use chrono::Utc;
    use rstest::{fixture, rstest};

    use super::*;

    #[fixture]
    fn valid_row() -> OfflineBundleRow {
        let now = Utc::now();
        OfflineBundleRow {
            id: Uuid::new_v4(),
            owner_user_id: Some(Uuid::new_v4()),
            device_id: "ios-phone".to_owned(),
            kind: "route".to_owned(),
            route_id: Some(Uuid::new_v4()),
            region_id: None,
            bounds: vec![-3.2, 55.9, -3.0, 56.0],
            min_zoom: 10,
            max_zoom: 16,
            estimated_size_bytes: 12_000,
            created_at: now,
            updated_at: now,
            status: "queued".to_owned(),
            progress: 0.0,
        }
    }

    #[rstest]
    fn pool_error_maps_to_connection_error() {
        let pool_err = PoolError::checkout("connection refused");
        let repo_err = map_pool_error(pool_err);

        assert!(matches!(
            repo_err,
            OfflineBundleRepositoryError::Connection { .. }
        ));
        assert!(repo_err.to_string().contains("connection refused"));
    }

    #[rstest]
    fn diesel_error_maps_to_query_error() {
        let diesel_err = diesel::result::Error::NotFound;
        let repo_err = map_diesel_error(diesel_err);

        assert!(matches!(
            repo_err,
            OfflineBundleRepositoryError::Query { .. }
        ));
        assert!(repo_err.to_string().contains("record not found"));
    }

    #[rstest]
    fn row_conversion_rejects_invalid_bounds_cardinality(mut valid_row: OfflineBundleRow) {
        valid_row.bounds = vec![-3.2, 55.9, -3.0];

        let error = row_to_offline_bundle(valid_row).expect_err("invalid bounds should fail");
        assert!(matches!(error, OfflineBundleRepositoryError::Query { .. }));
        assert!(error.to_string().contains("bounds expected 4 values"));
    }

    #[rstest]
    fn row_conversion_rejects_invalid_kind(mut valid_row: OfflineBundleRow) {
        valid_row.kind = "bogus".to_owned();

        let error = row_to_offline_bundle(valid_row).expect_err("invalid kind should fail");
        assert!(matches!(error, OfflineBundleRepositoryError::Query { .. }));
        assert!(error.to_string().contains("invalid offline bundle kind"));
    }
}
