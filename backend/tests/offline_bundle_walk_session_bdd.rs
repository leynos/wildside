//! Behavioural tests for offline bundle and walk session repositories.
use backend::domain::ports::{
    OfflineBundleRepository, OfflineBundleRepositoryError, WalkSessionRepository,
    WalkSessionRepositoryError,
};
use backend::domain::{OfflineBundle, UserId, WalkCompletionSummary, WalkSession};
use futures::executor::block_on;
use pg_embedded_setup_unpriv::TemporaryDatabase;
use postgres::{Client, NoTls};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use std::sync::{Arc, Mutex};

mod offline_bundle_walk_session_bdd {
    //! Split helpers for offline-bundle and walk-session behavioural tests.
    pub mod contract_checks;
    pub mod repository_impl;
    pub mod test_data;
}
mod support;

use offline_bundle_walk_session_bdd::contract_checks::{
    assert_offline_delete_and_lookup_contract, assert_walk_lookup_and_summary_filtering_contract,
};
use offline_bundle_walk_session_bdd::repository_impl::{
    PgOfflineBundleRepository, PgWalkSessionRepository, drop_table,
};
use offline_bundle_walk_session_bdd::test_data::{
    build_region_bundle, build_route_bundle, build_walk_session,
};
use support::atexit_cleanup::shared_cluster_handle;
use support::{format_postgres_error, handle_cluster_setup_failure, provision_template_database};

struct TestContext {
    offline_repo: PgOfflineBundleRepository,
    walk_repo: PgWalkSessionRepository,
    route_bundle: OfflineBundle,
    region_bundle: OfflineBundle,
    walk_session: WalkSession,
    database_url: String,
    last_offline_bundles: Option<Vec<OfflineBundle>>,
    last_walk_summaries: Option<Vec<WalkCompletionSummary>>,
    last_found_session: Option<Option<WalkSession>>,
    last_offline_error: Option<OfflineBundleRepositoryError>,
    last_walk_error: Option<WalkSessionRepositoryError>,
    _database: TemporaryDatabase,
}

type SharedContext = Arc<Mutex<TestContext>>;

fn seed_user_and_route(
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

fn setup_test_context() -> Result<TestContext, String> {
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

#[fixture]
fn world() -> SharedContext {
    match setup_test_context() {
        Ok(ctx) => Arc::new(Mutex::new(ctx)),
        Err(reason) => {
            let _: Option<()> = handle_cluster_setup_failure(reason);
            panic!("SKIP-TEST-CLUSTER");
        }
    }
}

fn assert_offline_success_and_get_bundles<'a>(
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

fn assert_single_bundle_matches(
    bundles: &[OfflineBundle],
    expected_id: &uuid::Uuid,
    expected_progress: f32,
) {
    assert_eq!(bundles.len(), 1);
    assert_eq!(bundles[0].id(), *expected_id);
    assert_eq!(bundles[0].progress(), expected_progress);
}

fn drop_table_and_save<T, E, F>(database_url: &str, table_name: &str, save_fn: F) -> Option<E>
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
fn execute_drop_table_save_scenario<T, E, Repo, Entity>(
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

#[given("postgres-backed offline bundle and walk session repositories")]
fn postgres_backed_offline_bundle_and_walk_session_repositories(world: SharedContext) {
    drop(world);
}

#[when("a route bundle and an anonymous region bundle are saved")]
fn a_route_bundle_and_anonymous_region_bundle_are_saved(world: SharedContext) {
    let (offline_repo, route_bundle, region_bundle) = {
        let ctx = world.lock().expect("context lock");
        (
            ctx.offline_repo.clone(),
            ctx.route_bundle.clone(),
            ctx.region_bundle.clone(),
        )
    };

    let result = block_on(async {
        offline_repo.save(&route_bundle).await?;
        offline_repo.save(&region_bundle).await
    });

    world.lock().expect("context lock").last_offline_error = result.err();
}

#[when("bundles are listed for the owner and device")]
fn bundles_are_listed_for_the_owner_and_device(world: SharedContext) {
    if let Some(save_error) = {
        let ctx = world.lock().expect("context lock");
        ctx.last_offline_error.clone()
    } {
        panic!("offline bundle save failed before listing: {save_error}");
    }

    let (offline_repo, owner_user_id, device_id) = {
        let ctx = world.lock().expect("context lock");
        (
            ctx.offline_repo.clone(),
            ctx.route_bundle.owner_user_id().cloned(),
            ctx.route_bundle.device_id().to_owned(),
        )
    };

    let result = block_on(async {
        offline_repo
            .list_for_owner_and_device(owner_user_id, device_id.as_str())
            .await
    });

    let mut ctx = world.lock().expect("context lock");
    match result {
        Ok(value) => {
            ctx.last_offline_bundles = Some(value);
            ctx.last_offline_error = None;
        }
        Err(err) => ctx.last_offline_error = Some(err),
    }
}

#[then("the owner listing includes the route bundle only")]
fn the_owner_listing_includes_the_route_bundle_only(world: SharedContext) {
    let ctx = world.lock().expect("context lock");
    let bundles = assert_offline_success_and_get_bundles(&ctx, "validating owner bundle listing");
    assert_single_bundle_matches(bundles, &ctx.route_bundle.id(), 1.0_f32);
}

#[when("anonymous bundles are listed for the region device")]
fn anonymous_bundles_are_listed_for_the_region_device(world: SharedContext) {
    let (offline_repo, device_id) = {
        let ctx = world.lock().expect("context lock");
        (
            ctx.offline_repo.clone(),
            ctx.region_bundle.device_id().to_owned(),
        )
    };

    let result = block_on(async {
        offline_repo
            .list_for_owner_and_device(None, device_id.as_str())
            .await
    });

    let mut ctx = world.lock().expect("context lock");
    match result {
        Ok(value) => {
            ctx.last_offline_bundles = Some(value);
            ctx.last_offline_error = None;
        }
        Err(err) => ctx.last_offline_error = Some(err),
    }
}

#[then("the anonymous listing includes the region bundle only")]
fn the_anonymous_listing_includes_the_region_bundle_only(world: SharedContext) {
    let ctx = world.lock().expect("context lock");
    let bundles =
        assert_offline_success_and_get_bundles(&ctx, "validating anonymous bundle listing");
    assert_single_bundle_matches(bundles, &ctx.region_bundle.id(), 0.0_f32);
}

#[when("a completed walk session is saved and queried")]
fn a_completed_walk_session_is_saved_and_queried(world: SharedContext) {
    let (walk_repo, walk_session) = {
        let ctx = world.lock().expect("context lock");
        (ctx.walk_repo.clone(), ctx.walk_session.clone())
    };

    let (save_result, find_result, summaries_result) = block_on(async {
        let save_result = walk_repo.save(&walk_session).await;
        if save_result.is_err() {
            return (save_result, Ok(None), Ok(Vec::new()));
        }

        let find_result = walk_repo.find_by_id(&walk_session.id()).await;
        if find_result.is_err() {
            return (save_result, find_result, Ok(Vec::new()));
        }

        let summaries_result = walk_repo
            .list_completion_summaries_for_user(walk_session.user_id())
            .await;
        (save_result, find_result, summaries_result)
    });

    let mut ctx = world.lock().expect("context lock");
    if let Err(err) = save_result {
        ctx.last_walk_error = Some(err);
        return;
    }

    match find_result {
        Ok(found) => ctx.last_found_session = Some(found),
        Err(err) => {
            ctx.last_walk_error = Some(err);
            return;
        }
    }

    match summaries_result {
        Ok(summaries) => {
            ctx.last_walk_summaries = Some(summaries);
            ctx.last_walk_error = None;
        }
        Err(err) => ctx.last_walk_error = Some(err),
    }
}

#[then("the walk session and completion summary are returned")]
fn the_walk_session_and_completion_summary_are_returned(world: SharedContext) {
    let ctx = world.lock().expect("context lock");
    assert!(ctx.last_walk_error.is_none(), "{:?}", ctx.last_walk_error);

    let found = ctx
        .last_found_session
        .as_ref()
        .expect("find should execute")
        .as_ref()
        .expect("session should exist");
    assert_eq!(found.id(), ctx.walk_session.id());

    let summaries = ctx
        .last_walk_summaries
        .as_ref()
        .expect("summary list should execute");
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].session_id(), ctx.walk_session.id());
}

#[when("offline delete and missing lookup contracts are exercised")]
fn offline_delete_and_missing_lookup_contracts_are_exercised(world: SharedContext) {
    let (offline_repo, route_bundle) = {
        let ctx = world.lock().expect("context lock");
        (ctx.offline_repo.clone(), ctx.route_bundle.clone())
    };
    assert_offline_delete_and_lookup_contract(offline_repo, route_bundle);
}

#[when("walk missing lookup and completion summary filtering contracts are exercised")]
fn walk_missing_lookup_and_completion_summary_filtering_contracts_are_exercised(
    world: SharedContext,
) {
    let (walk_repo, walk_session) = {
        let ctx = world.lock().expect("context lock");
        (ctx.walk_repo.clone(), ctx.walk_session.clone())
    };
    assert_walk_lookup_and_summary_filtering_contract(walk_repo, walk_session);
}

#[when("the offline bundle table is dropped and an offline save is attempted")]
fn the_offline_bundle_table_is_dropped_and_an_offline_save_is_attempted(world: SharedContext) {
    execute_drop_table_save_scenario(
        world,
        "offline_bundles",
        |ctx| {
            (
                ctx.database_url.clone(),
                ctx.offline_repo.clone(),
                ctx.route_bundle.clone(),
            )
        },
        |offline_repo, route_bundle| block_on(async { offline_repo.save(&route_bundle).await }),
        |ctx, error| ctx.last_offline_error = error,
    );
}

#[then("the offline repository reports a query error")]
fn the_offline_repository_reports_a_query_error(world: SharedContext) {
    let ctx = world.lock().expect("context lock");
    assert!(matches!(
        ctx.last_offline_error,
        Some(OfflineBundleRepositoryError::Query { .. })
    ));
}

#[when("the walk session table is dropped and a walk save is attempted")]
fn the_walk_session_table_is_dropped_and_a_walk_save_is_attempted(world: SharedContext) {
    execute_drop_table_save_scenario(
        world,
        "walk_sessions",
        |ctx| {
            (
                ctx.database_url.clone(),
                ctx.walk_repo.clone(),
                ctx.walk_session.clone(),
            )
        },
        |walk_repo, walk_session| block_on(async { walk_repo.save(&walk_session).await }),
        |ctx, error| ctx.last_walk_error = error,
    );
}

#[then("the walk session repository reports a query error")]
fn the_walk_session_repository_reports_a_query_error(world: SharedContext) {
    let ctx = world.lock().expect("context lock");
    assert!(matches!(
        ctx.last_walk_error,
        Some(WalkSessionRepositoryError::Query { .. })
    ));
}

#[scenario(
    path = "tests/features/offline_bundle_walk_session.feature",
    name = "Repositories persist manifests and completion summaries with query-error mapping"
)]
fn repositories_persist_manifests_and_completion_summaries_with_query_error_mapping(
    world: SharedContext,
) {
    drop(world);
}
