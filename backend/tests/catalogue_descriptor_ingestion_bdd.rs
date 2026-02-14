//! Behavioural tests for catalogue and descriptor domain type ingestion.

use std::sync::{Arc, Mutex};

use backend::domain::ports::{
    CatalogueIngestionRepository, CatalogueIngestionRepositoryError, DescriptorIngestionRepository,
    DescriptorIngestionRepositoryError,
};
use backend::domain::{
    Badge, CommunityPick, RouteCategory, RouteCollection, RouteSummary, SafetyPreset, SafetyToggle,
    Tag, Theme, TrendingRouteHighlight,
};
use backend::outbound::persistence::{
    DbPool, DieselCatalogueIngestionRepository, DieselDescriptorIngestionRepository, PoolConfig,
};
use pg_embedded_setup_unpriv::TemporaryDatabase;
use postgres::{Client, NoTls};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use serde_json::Value;
use tokio::runtime::Runtime;
use uuid::Uuid;

#[path = "support/catalogue_descriptor_builders.rs"]
mod builders;
#[path = "support/catalogue_descriptor_snapshots.rs"]
mod snapshots;

#[path = "catalogue_descriptor_ingestion_bdd/assertions.rs"]
mod assertions;
#[path = "catalogue_descriptor_ingestion_bdd/error_steps.rs"]
mod error_steps;

mod support;

use assertions::{
    assert_badges_stored, assert_community_picks_stored, assert_image_asset_json_shape,
    assert_localizations_json_shape, assert_preset_toggle_ids_stored, assert_route_category_stored,
    assert_route_collections_stored, assert_safety_toggles_stored,
    assert_summary_localizations_and_hero_image_stored, assert_tag_stored, assert_themes_stored,
    assert_trending_highlights_stored,
};
use builders::{CURATOR_USER_ID, EDGE_COMMUNITY_PICK_ID, ROUTE_ID};
use snapshots::{build_edge_community_pick, build_ingestion_snapshots};
use support::atexit_cleanup::shared_cluster_handle;
use support::provision_template_database;

struct TestContext {
    runtime: Runtime,
    catalogue_repository: DieselCatalogueIngestionRepository,
    descriptor_repository: DieselDescriptorIngestionRepository,
    client: Client,
    last_catalogue_error: Option<CatalogueIngestionRepositoryError>,
    last_descriptor_error: Option<DescriptorIngestionRepositoryError>,
    _database: TemporaryDatabase,
}

type SharedContext = Arc<Mutex<TestContext>>;

fn setup_test_context() -> TestContext {
    let runtime = Runtime::new().expect("tokio runtime should initialize");
    let cluster = shared_cluster_handle().expect("embedded postgres cluster should be available");
    let temp_db =
        provision_template_database(cluster).expect("template database should be available");

    let config = PoolConfig::new(temp_db.url())
        .with_max_size(2)
        .with_min_idle(Some(1));
    let pool = runtime
        .block_on(async { DbPool::new(config).await })
        .expect("pool should initialize");

    let client = Client::connect(temp_db.url(), NoTls).expect("postgres client should connect");

    TestContext {
        runtime,
        catalogue_repository: DieselCatalogueIngestionRepository::new(pool.clone()),
        descriptor_repository: DieselDescriptorIngestionRepository::new(pool),
        client,
        last_catalogue_error: None,
        last_descriptor_error: None,
        _database: temp_db,
    }
}

#[fixture]
fn world() -> SharedContext {
    Arc::new(Mutex::new(setup_test_context()))
}

#[given("a Diesel-backed catalogue and descriptor ingestion repository")]
fn a_diesel_backed_catalogue_and_descriptor_ingestion_repository(world: SharedContext) {
    let mut ctx = world.lock().expect("context lock");

    ctx.client
        .execute(
            "INSERT INTO users (id, display_name) VALUES ($1, $2) ON CONFLICT (id) DO NOTHING",
            &[&CURATOR_USER_ID, &"Behaviour Curator"],
        )
        .expect("fixture user should exist");
    ctx.client
        .execute(
            "INSERT INTO routes (id, user_id, path, generation_params) VALUES ($1, $2, '((0,0),(1,1))'::path, '{}'::jsonb) ON CONFLICT (id) DO NOTHING",
            &[&ROUTE_ID, &CURATOR_USER_ID],
        )
        .expect("fixture route should exist");
}

fn create_catalogue_fixtures() -> (
    RouteCategory,
    Theme,
    RouteCollection,
    RouteSummary,
    TrendingRouteHighlight,
    CommunityPick,
) {
    let snapshots = build_ingestion_snapshots();
    (
        snapshots.category,
        snapshots.theme,
        snapshots.collection,
        snapshots.summary,
        snapshots.highlight,
        snapshots.pick,
    )
}

fn create_descriptor_fixtures() -> (Tag, Badge, SafetyToggle, SafetyPreset) {
    let snapshots = build_ingestion_snapshots();
    (
        snapshots.tag,
        snapshots.badge,
        snapshots.toggle,
        snapshots.preset,
    )
}

#[when("the repositories upsert validated catalogue and descriptor snapshots")]
fn upsert_validated_catalogue_and_descriptor_snapshots(world: SharedContext) {
    let (catalogue_repository, descriptor_repository, handle) = {
        let ctx = world.lock().expect("context lock");
        (
            ctx.catalogue_repository.clone(),
            ctx.descriptor_repository.clone(),
            ctx.runtime.handle().clone(),
        )
    };

    let (category, theme, collection, summary, highlight, pick) = create_catalogue_fixtures();
    let (tag, badge, toggle, preset) = create_descriptor_fixtures();

    let catalogue_result = handle.block_on(async {
        catalogue_repository
            .upsert_route_categories(std::slice::from_ref(&category))
            .await?;
        catalogue_repository
            .upsert_themes(std::slice::from_ref(&theme))
            .await?;
        catalogue_repository
            .upsert_route_collections(std::slice::from_ref(&collection))
            .await?;
        catalogue_repository
            .upsert_route_summaries(std::slice::from_ref(&summary))
            .await?;
        catalogue_repository
            .upsert_trending_highlights(std::slice::from_ref(&highlight))
            .await?;
        catalogue_repository
            .upsert_community_picks(std::slice::from_ref(&pick))
            .await?;
        Ok::<(), CatalogueIngestionRepositoryError>(())
    });

    let descriptor_result = handle.block_on(async {
        descriptor_repository
            .upsert_tags(std::slice::from_ref(&tag))
            .await?;
        descriptor_repository
            .upsert_badges(std::slice::from_ref(&badge))
            .await?;
        descriptor_repository
            .upsert_safety_toggles(std::slice::from_ref(&toggle))
            .await?;
        descriptor_repository
            .upsert_safety_presets(std::slice::from_ref(&preset))
            .await?;
        Ok::<(), DescriptorIngestionRepositoryError>(())
    });

    let mut ctx = world.lock().expect("context lock");
    ctx.last_catalogue_error = catalogue_result.err();
    ctx.last_descriptor_error = descriptor_result.err();
}

#[then("catalogue and descriptor rows are stored with localization and semantic icon keys")]
fn catalogue_and_descriptor_rows_are_stored(world: SharedContext) {
    let mut ctx = world.lock().expect("context lock");
    assert!(
        ctx.last_catalogue_error.is_none(),
        "expected catalogue upserts to succeed, got {:?}",
        ctx.last_catalogue_error
    );
    assert!(
        ctx.last_descriptor_error.is_none(),
        "expected descriptor upserts to succeed, got {:?}",
        ctx.last_descriptor_error
    );

    assert_route_category_stored(&mut ctx.client);
    assert_themes_stored(&mut ctx.client);
    assert_route_collections_stored(&mut ctx.client);
    assert_trending_highlights_stored(&mut ctx.client);
    assert_community_picks_stored(&mut ctx.client);
    assert_tag_stored(&mut ctx.client);
    assert_badges_stored(&mut ctx.client);
    assert_safety_toggles_stored(&mut ctx.client);
    assert_summary_localizations_and_hero_image_stored(&mut ctx.client);
    assert_preset_toggle_ids_stored(&mut ctx.client);
}

#[when("a community pick without route and user references is upserted")]
fn a_community_pick_without_route_and_user_references_is_upserted(world: SharedContext) {
    let (catalogue_repository, handle) = {
        let ctx = world.lock().expect("context lock");
        (
            ctx.catalogue_repository.clone(),
            ctx.runtime.handle().clone(),
        )
    };

    let pick = build_edge_community_pick();

    let result = handle.block_on(async {
        catalogue_repository
            .upsert_community_picks(std::slice::from_ref(&pick))
            .await
    });

    let mut ctx = world.lock().expect("context lock");
    ctx.last_catalogue_error = result.err();
}

#[then("the stored community pick keeps null route and user references")]
fn the_stored_community_pick_keeps_null_route_and_user_references(world: SharedContext) {
    let mut ctx = world.lock().expect("context lock");
    assert!(
        ctx.last_catalogue_error.is_none(),
        "expected edge upsert to succeed, got {:?}",
        ctx.last_catalogue_error
    );

    let row = ctx
        .client
        .query_one(
            "SELECT route_summary_id, user_id, localizations::text, curator_avatar::text, saves FROM community_picks WHERE id = $1",
            &[&EDGE_COMMUNITY_PICK_ID],
        )
        .expect("edge community pick row should exist");

    assert_eq!(row.get::<_, Option<Uuid>>(0), None);
    assert_eq!(row.get::<_, Option<Uuid>>(1), None);

    let localizations =
        serde_json::from_str::<Value>(&row.get::<_, String>(2)).expect("edge localizations JSON");
    assert_localizations_json_shape(
        &localizations,
        ("Edge pick", "Edge pick short", "Edge pick description"),
        (
            "Edge pick FR",
            "Edge pick FR court",
            "Edge pick FR description",
        ),
    );

    let curator_avatar =
        serde_json::from_str::<Value>(&row.get::<_, String>(3)).expect("curator avatar JSON");
    assert_image_asset_json_shape(
        &curator_avatar,
        "https://example.test/avatar-edge.jpg",
        "Curator avatar",
    );

    let saves = row.get::<_, i32>(4);
    assert_eq!(saves, 17);
}

#[scenario(
    path = "tests/features/catalogue_descriptor_ingestion.feature",
    name = "Catalogue and descriptor ingestion supports success failure and nullable edge cases"
)]
fn catalogue_and_descriptor_ingestion_supports_success_failure_and_nullable_edge_cases(
    world: SharedContext,
) {
    drop(world);
}
