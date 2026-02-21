//! Shared BDD context and helper functions for repository step modules.

use backend::domain::ports::{OfflineBundleRepositoryError, WalkSessionRepositoryError};
use backend::domain::{OfflineBundle, UserId, WalkCompletionSummary, WalkSession};
use pg_embedded_setup_unpriv::TemporaryDatabase;
use postgres::{Client, NoTls};
use std::sync::{Arc, Mutex};

use super::repository_impl::{PgOfflineBundleRepository, PgWalkSessionRepository, drop_table};
use super::test_data::{build_region_bundle, build_route_bundle, build_walk_session};
use crate::support::atexit_cleanup::shared_cluster_handle;
use crate::support::{format_postgres_error, provision_template_database};

pub(crate) struct TestContext {
    pub(crate) offline_repo: PgOfflineBundleRepository,
    pub(crate) walk_repo: PgWalkSessionRepository,
    pub(crate) route_bundle: OfflineBundle,
    pub(crate) region_bundle: OfflineBundle,
    pub(crate) walk_session: WalkSession,
    pub(crate) database_url: String,
    pub(crate) last_offline_bundles: Option<Vec<OfflineBundle>>,
    pub(crate) last_walk_summaries: Option<Vec<WalkCompletionSummary>>,
    pub(crate) last_found_session: Option<Option<WalkSession>>,
    pub(crate) last_offline_error: Option<OfflineBundleRepositoryError>,
    pub(crate) last_walk_error: Option<WalkSessionRepositoryError>,
    pub(crate) _database: TemporaryDatabase,
}

pub(crate) type SharedContext = Arc<Mutex<TestContext>>;

pub(crate) fn seed_user_and_route(
    client: &mut Client,
    user_id: &UserId,
    route_id: uuid::Uuid,
) -> Result<(), String> {
    let user_uuid = *user_id.as_uuid();
    let display_name = "Offline BDD User";
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

pub(crate) fn setup_test_context() -> Result<TestContext, String> {
    let cluster = shared_cluster_handle().map_err(|err| err.to_string())?;
    let temporary_db = provision_template_database(cluster).map_err(|err| err.to_string())?;
    let database_url = temporary_db.url().to_owned();

    let mut client = Client::connect(temporary_db.url(), NoTls).map_err(|err| err.to_string())?;
    let user_id = UserId::random();
    let route_id = uuid::Uuid::new_v4();
    seed_user_and_route(&mut client, &user_id, route_id)?;

    let shared_client = Arc::new(Mutex::new(client));

    Ok(TestContext {
        offline_repo: PgOfflineBundleRepository::new(shared_client.clone()),
        walk_repo: PgWalkSessionRepository::new(shared_client),
        route_bundle: build_route_bundle(user_id.clone(), route_id),
        region_bundle: build_region_bundle(),
        walk_session: build_walk_session(user_id, route_id),
        database_url,
        last_offline_bundles: None,
        last_walk_summaries: None,
        last_found_session: None,
        last_offline_error: None,
        last_walk_error: None,
        _database: temporary_db,
    })
}

pub(crate) fn assert_offline_success_and_get_bundles<'a>(
    ctx: &'a TestContext,
    list_description: &str,
) -> &'a Vec<OfflineBundle> {
    assert!(
        ctx.last_offline_error.is_none(),
        "offline repository returned an error while {list_description}: {:?}",
        ctx.last_offline_error
    );

    assert!(
        ctx.last_offline_bundles.is_some(),
        "offline bundle list is missing while {list_description}; expected list operation to run"
    );

    ctx.last_offline_bundles
        .as_ref()
        .expect("offline bundle list should be available after assertion")
}

pub(crate) fn assert_single_bundle_matches(
    bundles: &[OfflineBundle],
    expected_id: &uuid::Uuid,
    expected_progress: f32,
) {
    assert_eq!(bundles.len(), 1);
    assert_eq!(bundles[0].id(), *expected_id);
    assert_eq!(bundles[0].progress(), expected_progress);
}

pub(crate) fn drop_table_and_save<T, E, F>(
    database_url: &str,
    table_name: &str,
    save_fn: F,
) -> Option<E>
where
    F: FnOnce() -> Result<T, E>,
{
    if let Err(err) = drop_table(database_url, table_name) {
        panic!("failed to drop table '{table_name}' before save attempt: {err}");
    }
    save_fn().err()
}

/// Executes a drop-table save scenario by extracting inputs and storing the save error.
#[expect(
    clippy::too_many_arguments,
    reason = "scenario helper accepts extraction, save, and storage closures explicitly"
)]
pub(crate) fn execute_drop_table_save_scenario<T, E, Repo, Entity>(
    world: SharedContext,
    table_name: &str,
    extract_fn: impl FnOnce(&TestContext) -> (String, Repo, Entity),
    save_fn: impl FnOnce(Repo, Entity) -> Result<T, E>,
    store_error_fn: impl FnOnce(&mut TestContext, Option<E>),
) {
    let (database_url, repo, entity) = {
        let ctx = world.lock().expect("context lock");
        extract_fn(&ctx)
    };

    let error = drop_table_and_save(&database_url, table_name, || save_fn(repo, entity));

    let mut ctx = world.lock().expect("context lock");
    store_error_fn(&mut ctx, error);
}
