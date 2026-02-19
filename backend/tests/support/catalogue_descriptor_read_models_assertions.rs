//! Shared assertion helpers for catalogue and descriptor read-model BDD tests.

use backend::domain::ports::{DescriptorSnapshot, ExploreCatalogueSnapshot};

use crate::builders::SAFETY_TOGGLE_ID;
use crate::{SharedContext, TestContext};

/// Assert that the snapshot contains the expected category payload.
///
/// # Examples
///
/// ```no_run
/// use backend::domain::ports::ExploreCatalogueSnapshot;
///
/// let snapshot = ExploreCatalogueSnapshot::empty();
/// // Panics because the helper expects one seeded category.
/// assert_categories(&snapshot);
/// ```
pub(crate) fn assert_categories(snapshot: &ExploreCatalogueSnapshot) {
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

/// Assert that the snapshot contains the expected theme payload.
///
/// # Examples
///
/// ```no_run
/// use backend::domain::ports::ExploreCatalogueSnapshot;
///
/// let snapshot = ExploreCatalogueSnapshot::empty();
/// // Panics because the helper expects one seeded theme.
/// assert_themes(&snapshot);
/// ```
pub(crate) fn assert_themes(snapshot: &ExploreCatalogueSnapshot) {
    assert_eq!(snapshot.themes.len(), 1, "expected 1 theme");
    assert_eq!(snapshot.themes[0].slug(), "coastal");
    assert_eq!(snapshot.themes[0].walk_count(), 23);
}

/// Assert that the snapshot contains the expected collection payload.
///
/// # Examples
///
/// ```no_run
/// use backend::domain::ports::ExploreCatalogueSnapshot;
///
/// let snapshot = ExploreCatalogueSnapshot::empty();
/// // Panics because the helper expects one seeded collection.
/// assert_collections(&snapshot);
/// ```
pub(crate) fn assert_collections(snapshot: &ExploreCatalogueSnapshot) {
    assert_eq!(snapshot.collections.len(), 1, "expected 1 collection");
    assert_eq!(snapshot.collections[0].slug(), "weekend-favourites");
}

/// Assert that the snapshot contains the expected route payload.
///
/// # Examples
///
/// ```no_run
/// use backend::domain::ports::ExploreCatalogueSnapshot;
///
/// let snapshot = ExploreCatalogueSnapshot::empty();
/// // Panics because the helper expects one seeded route summary.
/// assert_routes(&snapshot);
/// ```
pub(crate) fn assert_routes(snapshot: &ExploreCatalogueSnapshot) {
    assert_eq!(snapshot.routes.len(), 1, "expected 1 route summary");
    assert_eq!(snapshot.routes[0].slug(), Some("coastal-loop"));
    assert_eq!(snapshot.routes[0].distance_metres(), 4_500);
}

/// Assert that the snapshot contains the expected trending payload.
///
/// # Examples
///
/// ```no_run
/// use backend::domain::ports::ExploreCatalogueSnapshot;
///
/// let snapshot = ExploreCatalogueSnapshot::empty();
/// // Panics because the helper expects one seeded trending item.
/// assert_trending(&snapshot);
/// ```
pub(crate) fn assert_trending(snapshot: &ExploreCatalogueSnapshot) {
    assert_eq!(snapshot.trending.len(), 1, "expected 1 trending highlight");
    assert_eq!(snapshot.trending[0].trend_delta(), "+12%");
}

/// Assert that the snapshot contains the expected community pick payload.
///
/// # Examples
///
/// ```no_run
/// use backend::domain::ports::ExploreCatalogueSnapshot;
///
/// let snapshot = ExploreCatalogueSnapshot::empty();
/// // Panics because the helper expects a seeded community pick.
/// assert_community_pick(&snapshot);
/// ```
pub(crate) fn assert_community_pick(snapshot: &ExploreCatalogueSnapshot) {
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

/// Assert that all descriptor collections contain the expected seeded values.
///
/// # Examples
///
/// ```no_run
/// use backend::domain::ports::DescriptorSnapshot;
///
/// let snapshot = DescriptorSnapshot::empty();
/// // Panics because the helper expects seeded descriptor rows.
/// assert_descriptor_contents(&snapshot);
/// ```
pub(crate) fn assert_descriptor_contents(snapshot: &DescriptorSnapshot) {
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

/// Assert that all catalogue collections are empty.
///
/// # Examples
///
/// ```no_run
/// use backend::domain::ports::ExploreCatalogueSnapshot;
///
/// let snapshot = ExploreCatalogueSnapshot::empty();
/// // Succeeds because every collection is empty and `community_pick` is `None`.
/// assert_all_catalogue_collections_empty(&snapshot);
/// ```
pub(crate) fn assert_all_catalogue_collections_empty(snapshot: &ExploreCatalogueSnapshot) {
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

/// Generic helper to extract and unwrap a snapshot from the shared context.
///
/// # Examples
///
/// ```no_run
/// use backend::domain::ports::ExploreCatalogueSnapshot;
///
/// let world: SharedContext = todo!("populate SharedContext in test setup");
/// let _snapshot: ExploreCatalogueSnapshot =
///     get_snapshot(&world, |ctx| &ctx.last_catalogue_snapshot);
/// // Panics when the field is `None` or `Err`.
/// ```
fn get_snapshot<T, E>(
    world: &SharedContext,
    field_accessor: impl FnOnce(&TestContext) -> &Option<Result<T, E>>,
) -> T
where
    T: Clone,
    E: std::fmt::Debug,
{
    let ctx = world.lock().expect("context lock");
    field_accessor(&ctx)
        .as_ref()
        .expect("snapshot should be set")
        .as_ref()
        .expect("snapshot should be Ok")
        .clone()
}

pub(crate) fn get_catalogue_snapshot(world: &SharedContext) -> ExploreCatalogueSnapshot {
    get_snapshot(world, |ctx| &ctx.last_catalogue_snapshot)
}

pub(crate) fn get_descriptor_snapshot(world: &SharedContext) -> DescriptorSnapshot {
    get_snapshot(world, |ctx| &ctx.last_descriptor_snapshot)
}

pub(crate) fn assert_query_error<T, E>(
    world: &SharedContext,
    get_result: impl FnOnce(&TestContext) -> &Option<Result<T, E>>,
    is_query_variant: impl FnOnce(&Result<T, E>) -> bool,
) where
    T: std::fmt::Debug,
    E: std::fmt::Debug,
{
    let ctx = world.lock().expect("context lock");
    let result = get_result(&ctx).as_ref().expect("snapshot should be set");
    assert!(
        is_query_variant(result),
        "expected Query error, got {:?}",
        result
    );
}
