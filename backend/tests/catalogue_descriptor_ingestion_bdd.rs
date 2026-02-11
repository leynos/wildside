//! Behavioural tests for catalogue and descriptor domain type ingestion.

use std::future::Future;
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
use serde_json::{Value, json};
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
    assert_tag_stored(&mut ctx.client);
    assert_summary_localizations_and_hero_image_stored(&mut ctx.client);
    assert_preset_toggle_ids_stored(&mut ctx.client);
}

fn assert_route_category_stored(client: &mut Client) {
    let route_category = client
        .query_one(
            "SELECT slug, icon_key, localizations::text, route_count FROM route_categories WHERE id = $1",
            &[&ROUTE_CATEGORY_ID],
        )
        .expect("route category row should exist");
    assert_eq!(
        (
            route_category.get::<_, String>(0),
            route_category.get::<_, String>(1),
            route_category.get::<_, i32>(3),
        ),
        ("scenic".to_owned(), "category:scenic".to_owned(), 42),
    );
    let category_localizations = serde_json::from_str::<Value>(&route_category.get::<_, String>(2))
        .expect("route category localizations should parse");
    assert_localizations_json_shape(
        &category_localizations,
        ("Scenic", "Scenic short", "Scenic description"),
        ("Scenic FR", "Scenic FR court", "Scenic FR description"),
    );
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
    assert_localizations_json_shape(
        &tag_localizations,
        ("Family", "Family short", "Family description"),
        ("Family FR", "Family FR court", "Family FR description"),
    );
}

fn assert_summary_localizations_and_hero_image_stored(client: &mut Client) {
    let summary = client
        .query_one(
            "SELECT localizations::text, hero_image::text FROM route_summaries WHERE id = $1",
            &[&ROUTE_SUMMARY_ID],
        )
        .expect("route summary row should exist");
    let localizations =
        serde_json::from_str::<Value>(&summary.get::<_, String>(0)).expect("localizations JSON");
    assert_localizations_json_shape(
        &localizations,
        (
            "Coastal loop",
            "Coastal loop short",
            "Coastal loop description",
        ),
        (
            "Coastal loop FR",
            "Coastal loop FR court",
            "Coastal loop FR description",
        ),
    );

    let hero_image =
        serde_json::from_str::<Value>(&summary.get::<_, String>(1)).expect("hero image JSON");
    assert_image_asset_json_shape(&hero_image, "https://example.test/hero.jpg", "Hero image");
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

type LocalizedCopy<'a> = (&'a str, &'a str, &'a str);

fn assert_localizations_json_shape(
    localizations: &Value,
    en_gb: LocalizedCopy<'_>,
    fr_fr: LocalizedCopy<'_>,
) {
    let (en_name, en_short_label, en_description) = en_gb;
    let (fr_name, fr_short_label, fr_description) = fr_fr;

    let expected = json!({
        "en-GB": {
            "name": en_name,
            "shortLabel": en_short_label,
            "description": en_description,
        },
        "fr-FR": {
            "name": fr_name,
            "shortLabel": fr_short_label,
            "description": fr_description,
        },
    });
    assert_eq!(localizations, &expected);
    assert_localized_copy_for_locale(localizations, "en-GB", en_gb);
    assert_localized_copy_for_locale(localizations, "fr-FR", fr_fr);
}

fn assert_localized_copy_for_locale(
    localizations: &Value,
    locale: &str,
    expected_copy: LocalizedCopy<'_>,
) {
    let locale_json = localizations
        .get(locale)
        .expect("locale should exist")
        .as_object()
        .expect("locale value should be an object");
    assert_eq!(locale_json.len(), 3);

    let (name, short_label, description) = expected_copy;
    assert_eq!(
        locale_json
            .get("name")
            .expect("name key should exist")
            .as_str(),
        Some(name)
    );
    assert_eq!(
        locale_json
            .get("shortLabel")
            .expect("shortLabel key should exist")
            .as_str(),
        Some(short_label)
    );
    assert_eq!(
        locale_json
            .get("description")
            .expect("description key should exist")
            .as_str(),
        Some(description)
    );
}

fn assert_image_asset_json_shape(image: &Value, expected_url: &str, expected_alt: &str) {
    let image_json = image.as_object().expect("image value should be an object");
    assert_eq!(image_json.len(), 2);
    assert_eq!(
        image_json
            .get("url")
            .expect("url key should exist")
            .as_str(),
        Some(expected_url)
    );
    assert_eq!(
        image_json
            .get("alt")
            .expect("alt key should exist")
            .as_str(),
        Some(expected_alt)
    );
}

macro_rules! define_drop_table_upsert_step {
    (
        $fn_name:ident,
        $step_name:literal,
        $record_error_fn:ident,
        $repository_field:ident,
        $drop_sql:literal,
        $snapshot_field:ident,
        $upsert_method:ident
    ) => {
        #[when($step_name)]
        fn $fn_name(world: SharedContext) {
            $record_error_fn(
                &world,
                run_operation_with_dropped_table(
                    &world,
                    $drop_sql,
                    |ctx| ctx.$repository_field.clone(),
                    |repository| async move {
                        let value = build_ingestion_snapshots().$snapshot_field;
                        repository
                            .$upsert_method(std::slice::from_ref(&value))
                            .await
                    },
                ),
            );
        }
    };
}

define_drop_table_upsert_step!(
    the_tags_table_is_dropped_and_a_tag_upsert_is_attempted,
    "the tags table is dropped and a tag upsert is attempted",
    record_descriptor_error,
    descriptor_repository,
    "DROP TABLE tags;",
    tag,
    upsert_tags
);

#[then("the descriptor repository reports a query error")]
fn the_descriptor_repository_reports_a_query_error(world: SharedContext) {
    assert_world_query_error(
        &world,
        |ctx| &ctx.last_descriptor_error,
        |error| matches!(error, DescriptorIngestionRepositoryError::Query { .. }),
        "DescriptorIngestionRepositoryError::Query",
    );
}

define_drop_table_upsert_step!(
    the_route_categories_table_is_dropped_and_a_route_category_upsert_is_attempted,
    "the route categories table is dropped and a route category upsert is attempted",
    record_catalogue_error,
    catalogue_repository,
    "DROP TABLE route_categories CASCADE;",
    category,
    upsert_route_categories
);

#[then("the catalogue repository reports a query error")]
fn the_catalogue_repository_reports_a_query_error(world: SharedContext) {
    assert_world_query_error(
        &world,
        |ctx| &ctx.last_catalogue_error,
        |error| matches!(error, CatalogueIngestionRepositoryError::Query { .. }),
        "CatalogueIngestionRepositoryError::Query",
    );
}

fn record_descriptor_error(
    world: &SharedContext,
    error: Option<DescriptorIngestionRepositoryError>,
) {
    let mut ctx = world.lock().expect("context lock");
    ctx.last_descriptor_error = error;
}

fn record_catalogue_error(world: &SharedContext, error: Option<CatalogueIngestionRepositoryError>) {
    let mut ctx = world.lock().expect("context lock");
    ctx.last_catalogue_error = error;
}

fn assert_world_query_error<Error, SelectError, IsQuery>(
    world: &SharedContext,
    select_error: SelectError,
    is_query_error: IsQuery,
    expected_error_label: &str,
) where
    Error: std::fmt::Debug,
    SelectError: FnOnce(&TestContext) -> &Option<Error>,
    IsQuery: FnOnce(&Error) -> bool,
{
    let ctx = world.lock().expect("context lock");
    let error = select_error(&ctx);

    assert!(
        error.as_ref().is_some_and(is_query_error),
        "expected {expected_error_label}, got {:?}",
        error
    );
}

fn run_operation_with_dropped_table<Repo, Error, SelectRepository, Op, Fut>(
    world: &SharedContext,
    drop_table_sql: &str,
    select_repository: SelectRepository,
    operation: Op,
) -> Option<Error>
where
    SelectRepository: FnOnce(&TestContext) -> Repo,
    Op: FnOnce(Repo) -> Fut,
    Fut: Future<Output = Result<(), Error>>,
{
    let (repository, handle) = {
        let mut ctx = world.lock().expect("context lock");
        ctx.client
            .batch_execute(drop_table_sql)
            .expect("table should drop");
        (select_repository(&ctx), ctx.runtime.handle().clone())
    };

    let result = handle.block_on(operation(repository));
    result.err()
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
