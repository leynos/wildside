//! Offline-bundle BDD step definitions.

use backend::domain::OfflineBundle;
use backend::domain::ports::{OfflineBundleRepository, OfflineBundleRepositoryError};
use futures::executor::block_on;
use rstest_bdd_macros::{given, then, when};

use super::contract_checks::assert_offline_delete_and_lookup_contract;
use super::repository_impl::PgOfflineBundleRepository;
use super::steps_helpers::{
    ScenarioHandlers, SharedContext, TestContext, assert_offline_success_and_get_bundles,
    assert_single_bundle_matches, execute_drop_table_save_scenario,
};

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
    let expected_bundle_id = ctx.route_bundle.id();
    assert_single_bundle_matches(bundles, &expected_bundle_id, 1.0_f32);
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
    let expected_bundle_id = ctx.region_bundle.id();
    assert_single_bundle_matches(bundles, &expected_bundle_id, 0.0_f32);
}

#[when("offline delete and missing lookup contracts are exercised")]
fn offline_delete_and_missing_lookup_contracts_are_exercised(world: SharedContext) {
    let (offline_repo, route_bundle) = {
        let ctx = world.lock().expect("context lock");
        (ctx.offline_repo.clone(), ctx.route_bundle.clone())
    };
    assert_offline_delete_and_lookup_contract(offline_repo, route_bundle);
}

#[when("the offline bundle table is dropped and an offline save is attempted")]
fn the_offline_bundle_table_is_dropped_and_an_offline_save_is_attempted(world: SharedContext) {
    execute_drop_table_save_scenario::<_, _, PgOfflineBundleRepository, OfflineBundle, _, _, _>(
        world,
        "offline_bundles",
        ScenarioHandlers {
            extract_fn: |ctx: &TestContext| {
                (
                    ctx.database_url.clone(),
                    ctx.offline_repo.clone(),
                    ctx.route_bundle.clone(),
                )
            },
            save_fn: |offline_repo: PgOfflineBundleRepository, route_bundle: OfflineBundle| {
                block_on(async { offline_repo.save(&route_bundle).await })
            },
            store_error_fn: |ctx: &mut TestContext, error: Option<OfflineBundleRepositoryError>| {
                ctx.last_offline_error = error;
            },
        },
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
