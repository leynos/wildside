//! Behavioural tests for catalogue and descriptor read-model repositories.

use std::sync::{Arc, Mutex};

use backend::domain::ports::DescriptorSnapshot;
use backend::domain::ports::{
    CatalogueIngestionRepository, CatalogueRepository, CatalogueRepositoryError,
    DescriptorIngestionRepository, DescriptorRepository, DescriptorRepositoryError,
    ExploreCatalogueSnapshot,
};
use backend::outbound::persistence::{
    DbPool, DieselCatalogueIngestionRepository, DieselCatalogueRepository,
    DieselDescriptorIngestionRepository, DieselDescriptorRepository, PoolConfig,
};
use pg_embedded_setup_unpriv::TemporaryDatabase;
use postgres::{Client, NoTls};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use tokio::runtime::Runtime;

#[path = "support/catalogue_descriptor_builders.rs"]
mod builders;
#[expect(
    dead_code,
    reason = "shared module; not all snapshot builders used by this test crate"
)]
#[path = "support/catalogue_descriptor_snapshots.rs"]
mod snapshots;

mod support;

use builders::{CURATOR_USER_ID, ROUTE_ID, SAFETY_TOGGLE_ID};
use snapshots::build_ingestion_snapshots;
use support::atexit_cleanup::shared_cluster_handle;
use support::provision_template_database;

struct TestContext {
    runtime: Runtime,
    catalogue_ingestion_repo: DieselCatalogueIngestionRepository,
    descriptor_ingestion_repo: DieselDescriptorIngestionRepository,
    catalogue_repo: DieselCatalogueRepository,
    descriptor_repo: DieselDescriptorRepository,
    client: Client,
    last_catalogue_snapshot: Option<Result<ExploreCatalogueSnapshot, CatalogueRepositoryError>>,
    last_descriptor_snapshot: Option<Result<DescriptorSnapshot, DescriptorRepositoryError>>,
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
        catalogue_ingestion_repo: DieselCatalogueIngestionRepository::new(pool.clone()),
        descriptor_ingestion_repo: DieselDescriptorIngestionRepository::new(pool.clone()),
        catalogue_repo: DieselCatalogueRepository::new(pool.clone()),
        descriptor_repo: DieselDescriptorRepository::new(pool),
        client,
        last_catalogue_snapshot: None,
        last_descriptor_snapshot: None,
        _database: temp_db,
    }
}

#[fixture]
fn world() -> SharedContext {
    Arc::new(Mutex::new(setup_test_context()))
}

#[given("a Diesel-backed catalogue and descriptor read repository")]
fn a_diesel_backed_catalogue_and_descriptor_read_repository(world: SharedContext) {
    let mut ctx = world.lock().expect("context lock");

    ctx.client
        .execute(
            "INSERT INTO users (id, display_name) VALUES ($1, $2) ON CONFLICT (id) DO NOTHING",
            &[&CURATOR_USER_ID, &"Behaviour Curator"],
        )
        .expect("fixture user should exist");
    ctx.client
        .execute(
            "INSERT INTO routes (id, user_id, path, generation_params) \
             VALUES ($1, $2, '((0,0),(1,1))'::path, '{}'::jsonb) \
             ON CONFLICT (id) DO NOTHING",
            &[&ROUTE_ID, &CURATOR_USER_ID],
        )
        .expect("fixture route should exist");
}

#[when("catalogue and descriptor data is seeded via ingestion")]
fn catalogue_and_descriptor_data_is_seeded_via_ingestion(world: SharedContext) {
    let snapshots = build_ingestion_snapshots();

    let (cat_repo, desc_repo, handle) = {
        let ctx = world.lock().expect("context lock");
        (
            ctx.catalogue_ingestion_repo.clone(),
            ctx.descriptor_ingestion_repo.clone(),
            ctx.runtime.handle().clone(),
        )
    };

    handle
        .block_on(async {
            cat_repo
                .upsert_route_categories(std::slice::from_ref(&snapshots.category))
                .await?;
            cat_repo
                .upsert_themes(std::slice::from_ref(&snapshots.theme))
                .await?;
            cat_repo
                .upsert_route_collections(std::slice::from_ref(&snapshots.collection))
                .await?;
            cat_repo
                .upsert_route_summaries(std::slice::from_ref(&snapshots.summary))
                .await?;
            cat_repo
                .upsert_trending_highlights(std::slice::from_ref(&snapshots.highlight))
                .await?;
            cat_repo
                .upsert_community_picks(std::slice::from_ref(&snapshots.pick))
                .await
        })
        .expect("catalogue ingestion should succeed");

    handle
        .block_on(async {
            desc_repo
                .upsert_tags(std::slice::from_ref(&snapshots.tag))
                .await?;
            desc_repo
                .upsert_badges(std::slice::from_ref(&snapshots.badge))
                .await?;
            desc_repo
                .upsert_safety_toggles(std::slice::from_ref(&snapshots.toggle))
                .await?;
            desc_repo
                .upsert_safety_presets(std::slice::from_ref(&snapshots.preset))
                .await
        })
        .expect("descriptor ingestion should succeed");
}

#[when("the catalogue snapshot is read")]
fn the_catalogue_snapshot_is_read(world: SharedContext) {
    let (repo, handle) = {
        let ctx = world.lock().expect("context lock");
        (ctx.catalogue_repo.clone(), ctx.runtime.handle().clone())
    };

    let result = handle.block_on(repo.explore_snapshot());

    let mut ctx = world.lock().expect("context lock");
    ctx.last_catalogue_snapshot = Some(result);
}

#[when("the descriptor snapshot is read")]
fn the_descriptor_snapshot_is_read(world: SharedContext) {
    let (repo, handle) = {
        let ctx = world.lock().expect("context lock");
        (ctx.descriptor_repo.clone(), ctx.runtime.handle().clone())
    };

    let result = handle.block_on(repo.descriptor_snapshot());

    let mut ctx = world.lock().expect("context lock");
    ctx.last_descriptor_snapshot = Some(result);
}

#[when("all catalogue tables are truncated")]
fn all_catalogue_tables_are_truncated(world: SharedContext) {
    let mut ctx = world.lock().expect("context lock");
    ctx.client
        .batch_execute(
            "TRUNCATE community_picks, trending_route_highlights, \
             route_summaries, route_collections, themes, route_categories CASCADE",
        )
        .expect("catalogue tables should truncate");
}

#[when("a malformed localization row is inserted directly")]
fn a_malformed_localization_row_is_inserted_directly(world: SharedContext) {
    let mut ctx = world.lock().expect("context lock");
    ctx.client
        .execute(
            "INSERT INTO route_categories (id, slug, icon_key, localizations, route_count) \
             VALUES (gen_random_uuid(), 'malformed', 'category:malformed', '{}'::jsonb, 0)",
            &[],
        )
        .expect("malformed row should insert");
}

#[when("a malformed descriptor localization row is inserted directly")]
fn a_malformed_descriptor_localization_row_is_inserted_directly(world: SharedContext) {
    let mut ctx = world.lock().expect("context lock");
    ctx.client
        .execute(
            "INSERT INTO tags (id, slug, icon_key, localizations) \
             VALUES (gen_random_uuid(), 'malformed', 'tag:malformed', '{}'::jsonb)",
            &[],
        )
        .expect("malformed descriptor row should insert");
}

fn assert_categories(snapshot: &ExploreCatalogueSnapshot) {
    assert_eq!(snapshot.categories.len(), 1, "expected 1 category");
    assert_eq!(snapshot.categories[0].slug(), "scenic");
    assert_eq!(snapshot.categories[0].route_count(), 42);
    assert!(
        snapshot.categories[0]
            .localizations()
            .as_map()
            .contains_key("en-GB"),
        "category should have en-GB locale"
    );
}

fn assert_themes(snapshot: &ExploreCatalogueSnapshot) {
    assert_eq!(snapshot.themes.len(), 1, "expected 1 theme");
    assert_eq!(snapshot.themes[0].slug(), "coastal");
    assert_eq!(snapshot.themes[0].walk_count(), 23);
}

fn assert_collections(snapshot: &ExploreCatalogueSnapshot) {
    assert_eq!(snapshot.collections.len(), 1, "expected 1 collection");
    assert_eq!(snapshot.collections[0].slug(), "weekend-favourites");
}

fn assert_routes(snapshot: &ExploreCatalogueSnapshot) {
    assert_eq!(snapshot.routes.len(), 1, "expected 1 route summary");
    assert_eq!(snapshot.routes[0].slug(), Some("coastal-loop"));
    assert_eq!(snapshot.routes[0].distance_metres(), 4_500);
}

fn assert_trending(snapshot: &ExploreCatalogueSnapshot) {
    assert_eq!(snapshot.trending.len(), 1, "expected 1 trending highlight");
    assert_eq!(snapshot.trending[0].trend_delta(), "+12%");
}

fn assert_community_pick(snapshot: &ExploreCatalogueSnapshot) {
    let pick = snapshot
        .community_pick
        .as_ref()
        .expect("community pick should be present");
    assert_eq!(pick.curator_display_name(), "Wildside curators");
    assert_eq!(pick.saves(), 128);
    let locales = pick.localizations().as_map();
    assert!(
        locales.contains_key("en-GB"),
        "community pick should have en-GB locale"
    );
    assert!(
        locales.contains_key("fr-FR"),
        "community pick should have fr-FR locale"
    );
}

fn assert_descriptor_contents(snapshot: &DescriptorSnapshot) {
    assert_eq!(snapshot.tags.len(), 1, "expected 1 tag");
    assert_eq!(snapshot.tags[0].slug(), "family-friendly");
    assert_eq!(snapshot.badges.len(), 1, "expected 1 badge");
    assert_eq!(snapshot.badges[0].slug(), "accessible");
    assert_eq!(snapshot.safety_toggles.len(), 1, "expected 1 safety toggle");
    assert_eq!(snapshot.safety_toggles[0].slug(), "well-lit");
    assert_eq!(snapshot.safety_presets.len(), 1, "expected 1 safety preset");
    assert_eq!(snapshot.safety_presets[0].slug(), "night-safe");
    assert_eq!(
        snapshot.safety_presets[0].safety_toggle_ids(),
        &[SAFETY_TOGGLE_ID]
    );
}

fn assert_all_catalogue_collections_empty(snapshot: &ExploreCatalogueSnapshot) {
    assert!(snapshot.categories.is_empty(), "categories should be empty");
    assert!(snapshot.themes.is_empty(), "themes should be empty");
    assert!(
        snapshot.collections.is_empty(),
        "collections should be empty"
    );
    assert!(snapshot.routes.is_empty(), "routes should be empty");
    assert!(snapshot.trending.is_empty(), "trending should be empty");
    assert!(
        snapshot.community_pick.is_none(),
        "community pick should be None"
    );
}

fn get_catalogue_snapshot(world: &SharedContext) -> ExploreCatalogueSnapshot {
    world
        .lock()
        .expect("context lock")
        .last_catalogue_snapshot
        .as_ref()
        .expect("snapshot should be set")
        .as_ref()
        .expect("snapshot should be Ok")
        .clone()
}

fn get_descriptor_snapshot(world: &SharedContext) -> DescriptorSnapshot {
    world
        .lock()
        .expect("context lock")
        .last_descriptor_snapshot
        .as_ref()
        .expect("snapshot should be set")
        .as_ref()
        .expect("snapshot should be Ok")
        .clone()
}

#[then("the explore snapshot contains expected categories themes and routes")]
fn the_explore_snapshot_contains_expected_categories_themes_and_routes(world: SharedContext) {
    let snapshot = get_catalogue_snapshot(&world);
    assert_categories(&snapshot);
    assert_themes(&snapshot);
    assert_collections(&snapshot);
    assert_routes(&snapshot);
    assert_trending(&snapshot);
}

#[then("the community pick is present with correct localization")]
fn the_community_pick_is_present_with_correct_localization(world: SharedContext) {
    let snapshot = get_catalogue_snapshot(&world);
    assert_community_pick(&snapshot);
}

#[then("the descriptor snapshot contains expected tags badges and presets")]
fn the_descriptor_snapshot_contains_expected_tags_badges_and_presets(world: SharedContext) {
    let snapshot = get_descriptor_snapshot(&world);
    assert_descriptor_contents(&snapshot);
}

#[then("the explore snapshot returns empty collections")]
fn the_explore_snapshot_returns_empty_collections(world: SharedContext) {
    let snapshot = get_catalogue_snapshot(&world);
    assert_all_catalogue_collections_empty(&snapshot);
}

#[then("the catalogue read repository reports a query error")]
fn the_catalogue_read_repository_reports_a_query_error(world: SharedContext) {
    let ctx = world.lock().expect("context lock");
    let result = ctx
        .last_catalogue_snapshot
        .as_ref()
        .expect("snapshot should be set");
    assert!(
        matches!(result, Err(CatalogueRepositoryError::Query { .. })),
        "expected CatalogueRepositoryError::Query, got {:?}",
        result
    );
}

#[then("the descriptor read repository reports a query error")]
fn the_descriptor_read_repository_reports_a_query_error(world: SharedContext) {
    let ctx = world.lock().expect("context lock");
    let result = ctx
        .last_descriptor_snapshot
        .as_ref()
        .expect("snapshot should be set");
    assert!(
        matches!(result, Err(DescriptorRepositoryError::Query { .. })),
        "expected DescriptorRepositoryError::Query, got {:?}",
        result
    );
}

#[scenario(
    path = "tests/features/catalogue_descriptor_read_models.feature",
    name = "Read repositories return seeded snapshots and handle empty and malformed data"
)]
fn read_repositories_return_seeded_snapshots_and_handle_empty_and_malformed_data(
    world: SharedContext,
) {
    drop(world);
}
