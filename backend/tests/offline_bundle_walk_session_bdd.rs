//! Behavioural tests for offline bundle and walk session repositories.

use std::sync::{Arc, Mutex};

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

mod offline_bundle_walk_session_bdd {
    //! Split helpers for offline-bundle and walk-session behavioural tests.

    pub mod repository_impl;
    pub mod test_data;
}
mod support;

use offline_bundle_walk_session_bdd::repository_impl::{
    PgOfflineBundleRepository, PgWalkSessionRepository, create_contract_tables, drop_table,
};
use offline_bundle_walk_session_bdd::test_data::{
    build_region_bundle, build_route_bundle, build_walk_session,
};
use support::atexit_cleanup::shared_cluster_handle;
use support::{handle_cluster_setup_failure, provision_template_database};

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

fn setup_test_context() -> Result<TestContext, String> {
    let cluster = shared_cluster_handle().map_err(|err| err.to_string())?;
    let temporary_db = provision_template_database(cluster).map_err(|err| err.to_string())?;
    let database_url = temporary_db.url().to_owned();

    let mut client = Client::connect(temporary_db.url(), NoTls).map_err(|err| err.to_string())?;
    create_contract_tables(&mut client).map_err(|err| err.to_string())?;

    let shared_client = Arc::new(Mutex::new(client));
    let user_id = UserId::random();
    let route_id = uuid::Uuid::new_v4();

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
    assert!(
        ctx.last_offline_error.is_none(),
        "{:?}",
        ctx.last_offline_error
    );

    let bundles = ctx
        .last_offline_bundles
        .as_ref()
        .expect("owner list should execute");
    assert_eq!(bundles.len(), 1);
    assert_eq!(bundles[0].id(), ctx.route_bundle.id());
    assert_eq!(bundles[0].progress(), 1.0);
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
    assert!(
        ctx.last_offline_error.is_none(),
        "{:?}",
        ctx.last_offline_error
    );

    let bundles = ctx
        .last_offline_bundles
        .as_ref()
        .expect("anonymous list should execute");
    assert_eq!(bundles.len(), 1);
    assert_eq!(bundles[0].id(), ctx.region_bundle.id());
    assert_eq!(bundles[0].progress(), 0.0);
}

#[when("a completed walk session is saved and queried")]
fn a_completed_walk_session_is_saved_and_queried(world: SharedContext) {
    let (walk_repo, walk_session) = {
        let ctx = world.lock().expect("context lock");
        (ctx.walk_repo.clone(), ctx.walk_session.clone())
    };

    let save_result = block_on(async { walk_repo.save(&walk_session).await });

    let mut ctx = world.lock().expect("context lock");
    ctx.last_walk_error = save_result.err();
    if ctx.last_walk_error.is_some() {
        return;
    }

    let find_result = block_on(async { walk_repo.find_by_id(&walk_session.id()).await });
    if let Ok(found) = find_result {
        ctx.last_found_session = Some(found);
    } else if let Err(err) = find_result {
        ctx.last_walk_error = Some(err);
        return;
    }

    let summaries_result = block_on(async {
        walk_repo
            .list_completion_summaries_for_user(walk_session.user_id())
            .await
    });

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

#[when("the offline bundle table is dropped and an offline save is attempted")]
fn the_offline_bundle_table_is_dropped_and_an_offline_save_is_attempted(world: SharedContext) {
    let (database_url, offline_repo, route_bundle) = {
        let ctx = world.lock().expect("context lock");
        (
            ctx.database_url.clone(),
            ctx.offline_repo.clone(),
            ctx.route_bundle.clone(),
        )
    };

    drop_table(&database_url, "offline_bundles").expect("drop offline table");
    let result = block_on(async { offline_repo.save(&route_bundle).await });

    world.lock().expect("context lock").last_offline_error = result.err();
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
    let (database_url, walk_repo, walk_session) = {
        let ctx = world.lock().expect("context lock");
        (
            ctx.database_url.clone(),
            ctx.walk_repo.clone(),
            ctx.walk_session.clone(),
        )
    };

    drop_table(&database_url, "walk_sessions").expect("drop walk table");
    let result = block_on(async { walk_repo.save(&walk_session).await });

    world.lock().expect("context lock").last_walk_error = result.err();
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
