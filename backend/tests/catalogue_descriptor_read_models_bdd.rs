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
#[path = "support/catalogue_descriptor_read_models_assertions.rs"]
mod read_models_assertions;
#[expect(
    dead_code,
    reason = "shared module; not all snapshot builders used by this test crate"
)]
#[path = "support/catalogue_descriptor_snapshots.rs"]
mod snapshots;

mod support;

use builders::{CURATOR_USER_ID, ROUTE_ID};
use read_models_assertions::{
    assert_all_catalogue_collections_empty, assert_categories, assert_collections,
    assert_community_pick, assert_descriptor_contents, assert_query_error, assert_routes,
    assert_themes, assert_trending, get_catalogue_snapshot, get_descriptor_snapshot,
};
use snapshots::build_ingestion_snapshots;
use support::atexit_cleanup::shared_cluster_handle;
use support::{handle_cluster_setup_failure, provision_template_database};

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

fn setup_test_context() -> Result<TestContext, String> {
    let runtime = Runtime::new().map_err(|e| e.to_string())?;
    let cluster = shared_cluster_handle().map_err(|e| e.to_string())?;
    let temp_db = provision_template_database(cluster).map_err(|e| e.to_string())?;

    let config = PoolConfig::new(temp_db.url())
        .with_max_size(2)
        .with_min_idle(Some(1));
    let pool = runtime
        .block_on(async { DbPool::new(config).await })
        .map_err(|e| e.to_string())?;

    let client = Client::connect(temp_db.url(), NoTls).map_err(|e| e.to_string())?;

    Ok(TestContext {
        runtime,
        catalogue_ingestion_repo: DieselCatalogueIngestionRepository::new(pool.clone()),
        descriptor_ingestion_repo: DieselDescriptorIngestionRepository::new(pool.clone()),
        catalogue_repo: DieselCatalogueRepository::new(pool.clone()),
        descriptor_repo: DieselDescriptorRepository::new(pool),
        client,
        last_catalogue_snapshot: None,
        last_descriptor_snapshot: None,
        _database: temp_db,
    })
}

#[fixture]
fn world() -> SharedContext {
    match setup_test_context() {
        Ok(ctx) => Arc::new(Mutex::new(ctx)),
        Err(reason) => {
            let _: Option<()> = handle_cluster_setup_failure(&reason);
            panic!("SKIP-TEST-CLUSTER: {reason}");
        }
    }
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

fn insert_malformed_localization(client: &mut Client, table: &str, columns: &str, values: &str) {
    let sql = format!("INSERT INTO {table} ({columns}) VALUES ({values})");
    client
        .execute(&sql, &[])
        .expect("malformed row should insert");
}

#[when("a malformed localization row is inserted directly")]
fn a_malformed_localization_row_is_inserted_directly(world: SharedContext) {
    let mut ctx = world.lock().expect("context lock");
    insert_malformed_localization(
        &mut ctx.client,
        "route_categories",
        "id, slug, icon_key, localizations, route_count",
        "gen_random_uuid(), 'malformed', 'category:malformed', '{}'::jsonb, 0",
    );
}

#[when("a malformed descriptor localization row is inserted directly")]
fn a_malformed_descriptor_localization_row_is_inserted_directly(world: SharedContext) {
    let mut ctx = world.lock().expect("context lock");
    insert_malformed_localization(
        &mut ctx.client,
        "tags",
        "id, slug, icon_key, localizations",
        "gen_random_uuid(), 'malformed', 'tag:malformed', '{}'::jsonb",
    );
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
    assert_query_error(
        &world,
        |ctx| &ctx.last_catalogue_snapshot,
        |result| matches!(result, Err(CatalogueRepositoryError::Query { .. })),
    );
}

#[then("the descriptor read repository reports a query error")]
fn the_descriptor_read_repository_reports_a_query_error(world: SharedContext) {
    assert_query_error(
        &world,
        |ctx| &ctx.last_descriptor_snapshot,
        |result| matches!(result, Err(DescriptorRepositoryError::Query { .. })),
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
