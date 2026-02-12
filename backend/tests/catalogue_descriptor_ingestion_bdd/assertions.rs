//! Assertion helpers for catalogue/descriptor ingestion behavioural tests.

use postgres::Client;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::builders::{
    BADGE_ID, COMMUNITY_PICK_ID, CURATOR_USER_ID, HIGHLIGHT_ID, ROUTE_CATEGORY_ID,
    ROUTE_COLLECTION_ID, ROUTE_COLLECTION_ROUTE_ID, ROUTE_SUMMARY_ID, SAFETY_PRESET_ID,
    SAFETY_TOGGLE_ID, TAG_ID, THEME_ID,
};

pub(super) fn assert_route_category_stored(client: &mut Client) {
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

/// Expected values for descriptor assertion.
struct DescriptorExpectations<'a> {
    slug: &'a str,
    icon_key: &'a str,
    en_gb: LocalizedCopy<'a>,
    fr_fr: LocalizedCopy<'a>,
}

fn assert_simple_descriptor_stored(
    client: &mut Client,
    table: &str,
    id: &Uuid,
    expectations: DescriptorExpectations<'_>,
) {
    let query = format!("SELECT slug, icon_key, localizations::text FROM {table} WHERE id = $1");
    let row = client
        .query_one(query.as_str(), &[id])
        .expect("descriptor row should exist");

    assert_eq!(row.get::<_, String>(0), expectations.slug);
    assert_eq!(row.get::<_, String>(1), expectations.icon_key);

    let localizations = serde_json::from_str::<Value>(&row.get::<_, String>(2))
        .expect("descriptor localizations should parse");
    assert_localizations_json_shape(&localizations, expectations.en_gb, expectations.fr_fr);
}

pub(super) fn assert_tag_stored(client: &mut Client) {
    assert_simple_descriptor_stored(
        client,
        "tags",
        &TAG_ID,
        DescriptorExpectations {
            slug: "family-friendly",
            icon_key: "tag:family",
            en_gb: ("Family", "Family short", "Family description"),
            fr_fr: ("Family FR", "Family FR court", "Family FR description"),
        },
    );
}

pub(super) fn assert_themes_stored(client: &mut Client) {
    let theme = client
        .query_one(
            "SELECT slug, icon_key, localizations::text, image::text, walk_count, distance_range_metres, rating FROM themes WHERE id = $1",
            &[&THEME_ID],
        )
        .expect("theme row should exist");
    assert_eq!(theme.get::<_, String>(0), "coastal");
    assert_eq!(theme.get::<_, String>(1), "theme:coastal");

    let localizations =
        serde_json::from_str::<Value>(&theme.get::<_, String>(2)).expect("theme localizations");
    assert_localizations_json_shape(
        &localizations,
        ("Coastal", "Coastal short", "Coastal description"),
        ("Coastal FR", "Coastal FR court", "Coastal FR description"),
    );

    let image = serde_json::from_str::<Value>(&theme.get::<_, String>(3)).expect("theme image");
    assert_image_asset_json_shape(&image, "https://example.test/theme.jpg", "Theme image");

    assert_eq!(theme.get::<_, i32>(4), 23);
    assert_eq!(theme.get::<_, Vec<i32>>(5), vec![1_500, 9_000]);
    assert!((theme.get::<_, f32>(6) - 4.6).abs() < f32::EPSILON);
}

pub(super) fn assert_route_collections_stored(client: &mut Client) {
    let collection = client
        .query_one(
            "SELECT slug, icon_key, localizations::text, lead_image::text, map_preview::text, distance_range_metres, duration_range_seconds, difficulty, route_ids FROM route_collections WHERE id = $1",
            &[&ROUTE_COLLECTION_ID],
        )
        .expect("route collection row should exist");
    assert_eq!(collection.get::<_, String>(0), "weekend-favourites");
    assert_eq!(collection.get::<_, String>(1), "collection:weekend");

    let localizations = serde_json::from_str::<Value>(&collection.get::<_, String>(2))
        .expect("route collection localizations");
    assert_localizations_json_shape(
        &localizations,
        (
            "Weekend favourites",
            "Weekend favourites short",
            "Weekend favourites description",
        ),
        (
            "Weekend favourites FR",
            "Weekend favourites FR court",
            "Weekend favourites FR description",
        ),
    );

    let lead_image =
        serde_json::from_str::<Value>(&collection.get::<_, String>(3)).expect("lead image");
    assert_image_asset_json_shape(&lead_image, "https://example.test/lead.jpg", "Lead image");
    let map_preview =
        serde_json::from_str::<Value>(&collection.get::<_, String>(4)).expect("map preview");
    assert_image_asset_json_shape(&map_preview, "https://example.test/map.jpg", "Map preview");

    assert_eq!(collection.get::<_, Vec<i32>>(5), vec![2_000, 12_000]);
    assert_eq!(collection.get::<_, Vec<i32>>(6), vec![1_800, 7_200]);
    assert_eq!(collection.get::<_, String>(7), "moderate");
    assert_eq!(
        collection.get::<_, Vec<Uuid>>(8),
        vec![ROUTE_COLLECTION_ROUTE_ID]
    );
}

pub(super) fn assert_trending_highlights_stored(client: &mut Client) {
    let highlight = client
        .query_one(
            "SELECT route_summary_id, trend_delta, subtitle_localizations::text FROM trending_route_highlights WHERE id = $1",
            &[&HIGHLIGHT_ID],
        )
        .expect("trending highlight row should exist");
    assert_eq!(highlight.get::<_, Uuid>(0), ROUTE_SUMMARY_ID);
    assert_eq!(highlight.get::<_, String>(1), "+12%");

    let localizations = serde_json::from_str::<Value>(&highlight.get::<_, String>(2))
        .expect("trending highlight localizations");
    assert_localizations_json_shape(
        &localizations,
        (
            "Trending up",
            "Trending up short",
            "Trending up description",
        ),
        (
            "Trending up FR",
            "Trending up FR court",
            "Trending up FR description",
        ),
    );
}

pub(super) fn assert_summary_localizations_and_hero_image_stored(client: &mut Client) {
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

pub(super) fn assert_community_picks_stored(client: &mut Client) {
    let pick = client
        .query_one(
            "SELECT route_summary_id, user_id, localizations::text, curator_avatar::text, rating, distance_metres, duration_seconds, saves FROM community_picks WHERE id = $1",
            &[&COMMUNITY_PICK_ID],
        )
        .expect("community pick row should exist");
    assert_eq!(pick.get::<_, Option<Uuid>>(0), Some(ROUTE_SUMMARY_ID));
    assert_eq!(pick.get::<_, Option<Uuid>>(1), Some(CURATOR_USER_ID));

    let localizations =
        serde_json::from_str::<Value>(&pick.get::<_, String>(2)).expect("community pick JSON");
    assert_localizations_json_shape(
        &localizations,
        (
            "Community favourite",
            "Community favourite short",
            "Community favourite description",
        ),
        (
            "Community favourite FR",
            "Community favourite FR court",
            "Community favourite FR description",
        ),
    );

    let avatar =
        serde_json::from_str::<Value>(&pick.get::<_, String>(3)).expect("community pick avatar");
    assert_image_asset_json_shape(&avatar, "https://example.test/avatar.jpg", "Curator avatar");
    assert!((pick.get::<_, f32>(4) - 4.4).abs() < f32::EPSILON);
    assert_eq!(pick.get::<_, i32>(5), 3_400);
    assert_eq!(pick.get::<_, i32>(6), 4_800);
    assert_eq!(pick.get::<_, i32>(7), 128);
}

pub(super) fn assert_badges_stored(client: &mut Client) {
    assert_simple_descriptor_stored(
        client,
        "badges",
        &BADGE_ID,
        DescriptorExpectations {
            slug: "accessible",
            icon_key: "badge:accessible",
            en_gb: ("Accessible", "Accessible short", "Accessible description"),
            fr_fr: (
                "Accessible FR",
                "Accessible FR court",
                "Accessible FR description",
            ),
        },
    );
}

pub(super) fn assert_safety_toggles_stored(client: &mut Client) {
    assert_simple_descriptor_stored(
        client,
        "safety_toggles",
        &SAFETY_TOGGLE_ID,
        DescriptorExpectations {
            slug: "well-lit",
            icon_key: "safety:well-lit",
            en_gb: ("Well lit", "Well lit short", "Well lit description"),
            fr_fr: (
                "Well lit FR",
                "Well lit FR court",
                "Well lit FR description",
            ),
        },
    );
}

pub(super) fn assert_preset_toggle_ids_stored(client: &mut Client) {
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

pub(super) fn assert_localizations_json_shape(
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

pub(super) fn assert_image_asset_json_shape(image: &Value, expected_url: &str, expected_alt: &str) {
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
