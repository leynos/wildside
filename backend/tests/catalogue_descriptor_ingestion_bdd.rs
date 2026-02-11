//! Behavioural tests for catalogue and descriptor domain type ingestion.

use std::sync::{Arc, Mutex};

use backend::domain::ports::{
    CatalogueIngestionRepository, CatalogueIngestionRepositoryError, DescriptorIngestionRepository,
    DescriptorIngestionRepositoryError,
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
#[path = "support/pg_embed.rs"]
mod pg_embed;
#[path = "support/catalogue_descriptor_snapshots.rs"]
mod snapshots;

mod support;

use builders::{
    CURATOR_USER_ID, EDGE_COMMUNITY_PICK_ID, ROUTE_CATEGORY_ID, ROUTE_ID, ROUTE_SUMMARY_ID,
    SAFETY_PRESET_ID, SAFETY_TOGGLE_ID, TAG_ID,
};
use pg_embed::shared_cluster;
use snapshots::{build_edge_community_pick, build_ingestion_snapshots};
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
    let cluster = shared_cluster().expect("embedded postgres cluster should be available");
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

    let snapshots = build_ingestion_snapshots();

    let catalogue_result = handle.block_on(async {
        catalogue_repository
            .upsert_route_categories(std::slice::from_ref(&snapshots.category))
            .await?;
        catalogue_repository
            .upsert_themes(std::slice::from_ref(&snapshots.theme))
            .await?;
        catalogue_repository
            .upsert_route_collections(std::slice::from_ref(&snapshots.collection))
            .await?;
        catalogue_repository
            .upsert_route_summaries(std::slice::from_ref(&snapshots.summary))
            .await?;
        catalogue_repository
            .upsert_trending_highlights(std::slice::from_ref(&snapshots.highlight))
            .await?;
        catalogue_repository
            .upsert_community_picks(std::slice::from_ref(&snapshots.pick))
            .await?;
        Ok::<(), CatalogueIngestionRepositoryError>(())
    });

    let descriptor_result = handle.block_on(async {
        descriptor_repository
            .upsert_tags(std::slice::from_ref(&snapshots.tag))
            .await?;
        descriptor_repository
            .upsert_badges(std::slice::from_ref(&snapshots.badge))
            .await?;
        descriptor_repository
            .upsert_safety_toggles(std::slice::from_ref(&snapshots.toggle))
            .await?;
        descriptor_repository
            .upsert_safety_presets(std::slice::from_ref(&snapshots.preset))
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
    assert_tag_stored(&mut ctx.client);
    assert_summary_hero_image_stored(&mut ctx.client);
    assert_preset_toggle_ids_stored(&mut ctx.client);
}

fn assert_route_category_stored(client: &mut Client) {
    let route_category = client
        .query_one(
            "SELECT slug, icon_key, localizations::text, route_count FROM route_categories WHERE id = $1",
            &[&ROUTE_CATEGORY_ID],
        )
        .expect("route category row should exist");
    assert_eq!(route_category.get::<_, String>(0), "scenic");
    assert_eq!(route_category.get::<_, String>(1), "category:scenic");
    assert_eq!(route_category.get::<_, i32>(3), 42);
    let category_localizations = serde_json::from_str::<Value>(&route_category.get::<_, String>(2))
        .expect("route category localizations should parse");
    assert_eq!(category_localizations["en-GB"]["name"], "Scenic");
    assert_eq!(
        category_localizations["en-GB"]["shortLabel"],
        "Scenic short"
    );
    assert_eq!(
        category_localizations["en-GB"]["description"],
        "Scenic description"
    );
    assert_eq!(category_localizations["fr-FR"]["name"], "Scenic FR");
}

fn assert_tag_stored(client: &mut Client) {
    let tag = client
        .query_one(
            "SELECT slug, icon_key, localizations::text FROM tags WHERE id = $1",
            &[&TAG_ID],
        )
        .expect("tag row should exist");
    assert_eq!(tag.get::<_, String>(0), "family-friendly");
    assert_eq!(tag.get::<_, String>(1), "tag:family");
    let tag_localizations = serde_json::from_str::<Value>(&tag.get::<_, String>(2))
        .expect("tag localizations should parse");
    assert_eq!(tag_localizations["en-GB"]["name"], "Family");
    assert_eq!(tag_localizations["fr-FR"]["name"], "Family FR");
}

fn assert_summary_hero_image_stored(client: &mut Client) {
    let summary = client
        .query_one(
            "SELECT hero_image::text FROM route_summaries WHERE id = $1",
            &[&ROUTE_SUMMARY_ID],
        )
        .expect("route summary row should exist");
    let hero_image =
        serde_json::from_str::<Value>(&summary.get::<_, String>(0)).expect("hero image JSON");
    assert_eq!(hero_image["url"], "https://example.test/hero.jpg");
    assert_eq!(hero_image["alt"], "Hero image");
}

fn assert_preset_toggle_ids_stored(client: &mut Client) {
    let preset = client
        .query_one(
            "SELECT safety_toggle_ids FROM safety_presets WHERE id = $1",
            &[&SAFETY_PRESET_ID],
        )
        .expect("safety preset row should exist");
    let toggle_ids = preset.get::<_, Vec<Uuid>>(0);
    assert_eq!(toggle_ids, vec![SAFETY_TOGGLE_ID]);
}

#[when("the tags table is dropped and a tag upsert is attempted")]
fn the_tags_table_is_dropped_and_a_tag_upsert_is_attempted(world: SharedContext) {
    let (descriptor_repository, handle) = {
        let mut ctx = world.lock().expect("context lock");
        ctx.client
            .batch_execute("DROP TABLE tags;")
            .expect("tags table should drop");
        (
            ctx.descriptor_repository.clone(),
            ctx.runtime.handle().clone(),
        )
    };

    let tag = build_ingestion_snapshots().tag;

    let result = handle.block_on(async {
        descriptor_repository
            .upsert_tags(std::slice::from_ref(&tag))
            .await
    });

    let mut ctx = world.lock().expect("context lock");
    ctx.last_descriptor_error = result.err();
}

#[then("the descriptor repository reports a query error")]
fn the_descriptor_repository_reports_a_query_error(world: SharedContext) {
    let ctx = world.lock().expect("context lock");
    assert!(
        matches!(
            ctx.last_descriptor_error,
            Some(DescriptorIngestionRepositoryError::Query { .. })
        ),
        "expected DescriptorIngestionRepositoryError::Query, got {:?}",
        ctx.last_descriptor_error
    );
}

#[when("the route categories table is dropped and a route category upsert is attempted")]
fn the_route_categories_table_is_dropped_and_a_route_category_upsert_is_attempted(
    world: SharedContext,
) {
    let (catalogue_repository, handle) = {
        let mut ctx = world.lock().expect("context lock");
        ctx.client
            .batch_execute("DROP TABLE route_categories CASCADE;")
            .expect("route_categories table should drop");
        (
            ctx.catalogue_repository.clone(),
            ctx.runtime.handle().clone(),
        )
    };

    let category = build_ingestion_snapshots().category;

    let result = handle.block_on(async {
        catalogue_repository
            .upsert_route_categories(std::slice::from_ref(&category))
            .await
    });

    let mut ctx = world.lock().expect("context lock");
    ctx.last_catalogue_error = result.err();
}

#[then("the catalogue repository reports a query error")]
fn the_catalogue_repository_reports_a_query_error(world: SharedContext) {
    let ctx = world.lock().expect("context lock");
    assert!(
        matches!(
            ctx.last_catalogue_error,
            Some(CatalogueIngestionRepositoryError::Query { .. })
        ),
        "expected CatalogueIngestionRepositoryError::Query, got {:?}",
        ctx.last_catalogue_error
    );
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
            "SELECT route_summary_id, user_id, localizations->'en-GB'->>'name', saves FROM community_picks WHERE id = $1",
            &[&EDGE_COMMUNITY_PICK_ID],
        )
        .expect("edge community pick row should exist");

    assert_eq!(row.get::<_, Option<Uuid>>(0), None);
    assert_eq!(row.get::<_, Option<Uuid>>(1), None);
    assert_eq!(row.get::<_, String>(2), "Edge pick");
    assert_eq!(row.get::<_, i32>(3), 0);
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
