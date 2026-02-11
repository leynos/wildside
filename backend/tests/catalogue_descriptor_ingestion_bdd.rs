//! Behavioural tests for catalogue and descriptor domain type ingestion.

use std::sync::{Arc, Mutex};

use backend::domain::ports::{
    CatalogueIngestionRepository, CatalogueIngestionRepositoryError, DescriptorIngestionRepository,
    DescriptorIngestionRepositoryError,
};
use backend::domain::{
    Badge, CommunityPick, CommunityPickDraft, RouteCategory, RouteCategoryDraft, RouteCollection,
    RouteCollectionDraft, RouteSummary, RouteSummaryDraft, SafetyPreset, SafetyPresetDraft,
    SafetyToggle, Tag, Theme, ThemeDraft, TrendingRouteHighlight,
};
use backend::outbound::persistence::{
    DbPool, DieselCatalogueIngestionRepository, DieselDescriptorIngestionRepository, PoolConfig,
};
use pg_embedded_setup_unpriv::TemporaryDatabase;
use postgres::{Client, NoTls};
use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};
use tokio::runtime::Runtime;
use uuid::Uuid;

#[path = "support/catalogue_descriptor_builders.rs"]
mod builders;
#[path = "support/pg_embed.rs"]
mod pg_embed;

mod support;

use builders::{
    BADGE_ID, COMMUNITY_PICK_ID, CURATOR_USER_ID, EDGE_COMMUNITY_PICK_ID, HIGHLIGHT_ID,
    ROUTE_CATEGORY_ID, ROUTE_COLLECTION_ID, ROUTE_ID, ROUTE_SUMMARY_ID, SAFETY_PRESET_ID,
    SAFETY_TOGGLE_ID, TAG_ID, THEME_ID, icon, image, localizations,
};
use pg_embed::shared_cluster;
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

    let category = RouteCategory::new(RouteCategoryDraft {
        id: ROUTE_CATEGORY_ID,
        slug: "scenic".to_owned(),
        icon_key: icon("category:scenic"),
        localizations: localizations("Scenic"),
        route_count: 42,
    })
    .expect("route category should be valid");

    let theme = Theme::new(ThemeDraft {
        id: THEME_ID,
        slug: "coastal".to_owned(),
        icon_key: icon("theme:coastal"),
        localizations: localizations("Coastal"),
        image: image("https://example.test/theme.jpg", "Theme image"),
        walk_count: 23,
        distance_range_metres: [1_500, 9_000],
        rating: 4.6,
    })
    .expect("theme should be valid");

    let collection = RouteCollection::new(RouteCollectionDraft {
        id: ROUTE_COLLECTION_ID,
        slug: "weekend-favourites".to_owned(),
        icon_key: icon("collection:weekend"),
        localizations: localizations("Weekend favourites"),
        lead_image: image("https://example.test/lead.jpg", "Lead image"),
        map_preview: image("https://example.test/map.jpg", "Map preview"),
        distance_range_metres: [2_000, 12_000],
        duration_range_seconds: [1_800, 7_200],
        difficulty: "moderate".to_owned(),
        route_ids: vec![Uuid::new_v4()],
    })
    .expect("route collection should be valid");

    let summary = RouteSummary::new(RouteSummaryDraft {
        id: ROUTE_SUMMARY_ID,
        route_id: ROUTE_ID,
        category_id: ROUTE_CATEGORY_ID,
        theme_id: THEME_ID,
        slug: Some("coastal-loop".to_owned()),
        localizations: localizations("Coastal loop"),
        hero_image: image("https://example.test/hero.jpg", "Hero image"),
        distance_metres: 4_500,
        duration_seconds: 5_400,
        rating: 4.5,
        badge_ids: vec![BADGE_ID],
        difficulty: "moderate".to_owned(),
        interest_theme_ids: vec![Uuid::new_v4()],
    })
    .expect("route summary should be valid");

    let highlight = TrendingRouteHighlight::new(
        HIGHLIGHT_ID,
        ROUTE_SUMMARY_ID,
        "+12%",
        localizations("Trending up"),
    )
    .expect("highlight should be valid");

    let pick = CommunityPick::new(CommunityPickDraft {
        id: COMMUNITY_PICK_ID,
        route_summary_id: Some(ROUTE_SUMMARY_ID),
        user_id: Some(CURATOR_USER_ID),
        localizations: localizations("Community favourite"),
        curator_display_name: "Wildside curators".to_owned(),
        curator_avatar: image("https://example.test/avatar.jpg", "Curator avatar"),
        rating: 4.4,
        distance_metres: 3_400,
        duration_seconds: 4_800,
        saves: 128,
    })
    .expect("community pick should be valid");

    let tag = Tag::new(
        TAG_ID,
        "family-friendly",
        icon("tag:family"),
        localizations("Family"),
    )
    .expect("tag should be valid");
    let badge = Badge::new(
        BADGE_ID,
        "accessible",
        icon("badge:accessible"),
        localizations("Accessible"),
    )
    .expect("badge should be valid");
    let toggle = SafetyToggle::new(
        SAFETY_TOGGLE_ID,
        "well-lit",
        icon("safety:well-lit"),
        localizations("Well lit"),
    )
    .expect("safety toggle should be valid");
    let preset = SafetyPreset::new(SafetyPresetDraft {
        id: SAFETY_PRESET_ID,
        slug: "night-safe".to_owned(),
        icon_key: icon("preset:night-safe"),
        localizations: localizations("Night safe"),
        safety_toggle_ids: vec![SAFETY_TOGGLE_ID],
    })
    .expect("safety preset should be valid");

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

    let route_category = ctx
        .client
        .query_one(
            "SELECT slug, icon_key, localizations->'en-GB'->>'name', route_count FROM route_categories WHERE id = $1",
            &[&ROUTE_CATEGORY_ID],
        )
        .expect("route category row should exist");
    assert_eq!(route_category.get::<_, String>(0), "scenic");
    assert_eq!(route_category.get::<_, String>(1), "category:scenic");
    assert_eq!(route_category.get::<_, String>(2), "Scenic");
    assert_eq!(route_category.get::<_, i32>(3), 42);

    let tag = ctx
        .client
        .query_one(
            "SELECT slug, icon_key, localizations->'en-GB'->>'name' FROM tags WHERE id = $1",
            &[&TAG_ID],
        )
        .expect("tag row should exist");
    assert_eq!(tag.get::<_, String>(0), "family-friendly");
    assert_eq!(tag.get::<_, String>(1), "tag:family");
    assert_eq!(tag.get::<_, String>(2), "Family");

    let preset = ctx
        .client
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

    let tag = Tag::new(
        TAG_ID,
        "family-friendly",
        icon("tag:family"),
        localizations("Family"),
    )
    .expect("tag should be valid");

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

#[when("a community pick without route and user references is upserted")]
fn a_community_pick_without_route_and_user_references_is_upserted(world: SharedContext) {
    let (catalogue_repository, handle) = {
        let ctx = world.lock().expect("context lock");
        (
            ctx.catalogue_repository.clone(),
            ctx.runtime.handle().clone(),
        )
    };

    let pick = CommunityPick::new(CommunityPickDraft {
        id: EDGE_COMMUNITY_PICK_ID,
        route_summary_id: None,
        user_id: None,
        localizations: localizations("Edge pick"),
        curator_display_name: "Wildside curators".to_owned(),
        curator_avatar: image("https://example.test/avatar-edge.jpg", "Curator avatar"),
        rating: 4.0,
        distance_metres: 1_250,
        duration_seconds: 2_100,
        saves: 0,
    })
    .expect("edge community pick should be valid");

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
            "SELECT route_summary_id, user_id, localizations->'en-GB'->>'name' FROM community_picks WHERE id = $1",
            &[&EDGE_COMMUNITY_PICK_ID],
        )
        .expect("edge community pick row should exist");

    assert_eq!(row.get::<_, Option<Uuid>>(0), None);
    assert_eq!(row.get::<_, Option<Uuid>>(1), None);
    assert_eq!(row.get::<_, String>(2), "Edge pick");
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
