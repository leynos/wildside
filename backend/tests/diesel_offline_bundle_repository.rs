//! Integration tests for `DieselOfflineBundleRepository`.
//!
//! This suite validates persistence behaviour against embedded PostgreSQL
//! using the shared `pg-embedded-setup-unpriv` fixture pattern.

use backend::domain::ports::{OfflineBundleRepository, OfflineBundleRepositoryError};
use backend::domain::{
    BoundingBox, OfflineBundle, OfflineBundleDraft, OfflineBundleKind, OfflineBundleStatus, UserId,
    ZoomRange,
};
use backend::outbound::persistence::{DbPool, DieselOfflineBundleRepository, PoolConfig};
use chrono::Utc;
use pg_embedded_setup_unpriv::TemporaryDatabase;
use postgres::{Client, NoTls};
use rstest::{fixture, rstest};
use tokio::runtime::Runtime;
use uuid::Uuid;

mod support;

use support::atexit_cleanup::shared_cluster_handle;
use support::{format_postgres_error, handle_cluster_setup_failure, provision_template_database};

struct TestContext {
    runtime: Runtime,
    repository: DieselOfflineBundleRepository,
    owner_user_id: UserId,
    route_id: Uuid,
    database_url: String,
    _database: TemporaryDatabase,
}

fn seed_user_and_route(url: &str, user_id: &UserId, route_id: Uuid) -> Result<(), String> {
    let mut client = Client::connect(url, NoTls).map_err(|err| format_postgres_error(&err))?;
    let user_uuid = *user_id.as_uuid();
    let display_name = "Offline Bundle Test User";

    client
        .execute(
            "INSERT INTO users (id, display_name) VALUES ($1, $2)",
            &[&user_uuid, &display_name],
        )
        .map_err(|err| format_postgres_error(&err))?;

    client
        .execute(
            concat!(
                "INSERT INTO routes (id, user_id, path, generation_params) ",
                "VALUES ($1, $2, '((0,0),(1,1))'::path, '{}'::jsonb)"
            ),
            &[&route_id, &user_uuid],
        )
        .map_err(|err| format_postgres_error(&err))?;

    Ok(())
}

#[expect(
    clippy::too_many_arguments,
    reason = "Test helper keeps each varying bundle field explicit for scenario clarity"
)]
fn build_offline_bundle(
    owner_user_id: Option<UserId>,
    device_id: &str,
    kind: OfflineBundleKind,
    route_id: Option<Uuid>,
    region_id: Option<String>,
    bounds: (f64, f64, f64, f64),
    zoom_range: (u8, u8),
    estimated_size_bytes: u64,
    status: OfflineBundleStatus,
    progress: f32,
) -> OfflineBundle {
    let now = Utc::now();
    OfflineBundle::new(OfflineBundleDraft {
        id: Uuid::new_v4(),
        owner_user_id,
        device_id: device_id.to_owned(),
        kind,
        route_id,
        region_id,
        bounds: BoundingBox::new(bounds.0, bounds.1, bounds.2, bounds.3).expect("valid bounds"),
        zoom_range: ZoomRange::new(zoom_range.0, zoom_range.1).expect("valid zoom range"),
        estimated_size_bytes,
        created_at: now,
        updated_at: now,
        status,
        progress,
    })
    .expect("valid offline bundle")
}

fn build_route_bundle(owner_user_id: UserId, route_id: Uuid) -> OfflineBundle {
    build_offline_bundle(
        Some(owner_user_id),
        "owner-phone",
        OfflineBundleKind::Route,
        Some(route_id),
        None,
        (-3.24, 55.92, -3.12, 55.99),
        (11, 15),
        42_000,
        OfflineBundleStatus::Complete,
        1.0,
    )
}

fn build_region_bundle() -> OfflineBundle {
    build_offline_bundle(
        None,
        "shared-tablet",
        OfflineBundleKind::Region,
        None,
        Some("edinburgh-old-town".to_owned()),
        (-3.22, 55.93, -3.16, 55.97),
        (10, 14),
        9_000,
        OfflineBundleStatus::Queued,
        0.0,
    )
}

fn drop_table(url: &str, table_name: &str) -> Result<(), String> {
    let mut client = Client::connect(url, NoTls).map_err(|err| format_postgres_error(&err))?;
    let escaped_name = table_name.replace('"', "\"\"");
    let sql = format!(r#"DROP TABLE IF EXISTS "{escaped_name}""#);
    client
        .batch_execute(sql.as_str())
        .map_err(|err| format_postgres_error(&err))
}

fn setup_context() -> Result<TestContext, String> {
    let runtime = Runtime::new().map_err(|err| err.to_string())?;
    let cluster = shared_cluster_handle().map_err(|e| e.to_string())?;
    let temp_db = provision_template_database(cluster).map_err(|err| err.to_string())?;
    let database_url = temp_db.url().to_string();

    let owner_user_id = UserId::random();
    let route_id = Uuid::new_v4();
    seed_user_and_route(database_url.as_str(), &owner_user_id, route_id)?;

    let config = PoolConfig::new(database_url.as_str())
        .with_max_size(2)
        .with_min_idle(Some(1));
    let pool = runtime
        .block_on(async { DbPool::new(config).await })
        .map_err(|err| err.to_string())?;

    let repository = DieselOfflineBundleRepository::new(pool);

    Ok(TestContext {
        runtime,
        repository,
        owner_user_id,
        route_id,
        database_url,
        _database: temp_db,
    })
}

#[fixture]
fn repo_context() -> Option<TestContext> {
    match setup_context() {
        Ok(ctx) => Some(ctx),
        Err(reason) => handle_cluster_setup_failure(reason),
    }
}

#[rstest]
fn offline_repository_save_find_list_delete_contract(repo_context: Option<TestContext>) {
    let Some(context) = repo_context else {
        eprintln!("SKIP-TEST-CLUSTER: offline_repository_save_find_list_delete_contract skipped");
        return;
    };

    let repository = context.repository.clone();
    let route_bundle = build_route_bundle(context.owner_user_id.clone(), context.route_id);
    let region_bundle = build_region_bundle();

    context.runtime.block_on(async {
        repository
            .save(&route_bundle)
            .await
            .expect("save route bundle");
        repository
            .save(&region_bundle)
            .await
            .expect("save region bundle");
    });

    let found_route = context
        .runtime
        .block_on(async { repository.find_by_id(&route_bundle.id()).await })
        .expect("find route bundle")
        .expect("route bundle exists");
    assert_eq!(found_route.id(), route_bundle.id());
    assert_eq!(found_route.status(), OfflineBundleStatus::Complete);

    let owner_list = context
        .runtime
        .block_on(async {
            repository
                .list_for_owner_and_device(
                    Some(context.owner_user_id.clone()),
                    route_bundle.device_id(),
                )
                .await
        })
        .expect("owner/device list succeeds");
    assert_eq!(owner_list.len(), 1);
    assert_eq!(owner_list[0].id(), route_bundle.id());

    let anonymous_list = context
        .runtime
        .block_on(async {
            repository
                .list_for_owner_and_device(None, region_bundle.device_id())
                .await
        })
        .expect("anonymous/device list succeeds");
    assert_eq!(anonymous_list.len(), 1);
    assert_eq!(anonymous_list[0].id(), region_bundle.id());

    let was_deleted = context
        .runtime
        .block_on(async { repository.delete(&route_bundle.id()).await })
        .expect("delete route bundle");
    assert!(was_deleted, "delete should report removed row");

    let deleted_lookup = context
        .runtime
        .block_on(async { repository.find_by_id(&route_bundle.id()).await })
        .expect("lookup after delete should succeed");
    assert!(
        deleted_lookup.is_none(),
        "deleted bundle lookup should be none"
    );

    let missing_delete = context
        .runtime
        .block_on(async { repository.delete(&Uuid::new_v4()).await })
        .expect("delete missing bundle should succeed");
    assert!(!missing_delete, "missing delete should report false");
}

#[rstest]
fn offline_repository_maps_missing_schema_to_query_error(repo_context: Option<TestContext>) {
    let Some(context) = repo_context else {
        eprintln!(
            "SKIP-TEST-CLUSTER: offline_repository_maps_missing_schema_to_query_error skipped"
        );
        return;
    };

    drop_table(context.database_url.as_str(), "offline_bundles").expect("drop table succeeds");

    let repository = context.repository.clone();
    let route_bundle = build_route_bundle(context.owner_user_id.clone(), context.route_id);
    let error = context
        .runtime
        .block_on(async { repository.save(&route_bundle).await })
        .expect_err("save should fail when table is missing");

    assert!(matches!(error, OfflineBundleRepositoryError::Query { .. }));
}
