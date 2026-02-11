//! Shared snapshot builders for catalogue/descriptor behavioural tests.

use backend::domain::{
    Badge, CommunityPick, CommunityPickDraft, RouteCategory, RouteCategoryDraft, RouteCollection,
    RouteCollectionDraft, RouteSummary, RouteSummaryDraft, SafetyPreset, SafetyPresetDraft,
    SafetyToggle, Tag, Theme, ThemeDraft, TrendingRouteHighlight,
};
use uuid::Uuid;

use crate::builders::{
    BADGE_ID, COMMUNITY_PICK_ID, CURATOR_USER_ID, EDGE_COMMUNITY_PICK_ID, HIGHLIGHT_ID,
    ROUTE_CATEGORY_ID, ROUTE_COLLECTION_ID, ROUTE_ID, ROUTE_SUMMARY_ID, SAFETY_PRESET_ID,
    SAFETY_TOGGLE_ID, TAG_ID, THEME_ID, icon, image, localizations,
};

pub(crate) struct IngestionSnapshots {
    pub(crate) category: RouteCategory,
    pub(crate) theme: Theme,
    pub(crate) collection: RouteCollection,
    pub(crate) summary: RouteSummary,
    pub(crate) highlight: TrendingRouteHighlight,
    pub(crate) pick: CommunityPick,
    pub(crate) tag: Tag,
    pub(crate) badge: Badge,
    pub(crate) toggle: SafetyToggle,
    pub(crate) preset: SafetyPreset,
}

pub(crate) fn build_ingestion_snapshots() -> IngestionSnapshots {
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

    IngestionSnapshots {
        category,
        theme,
        collection,
        summary,
        highlight,
        pick,
        tag,
        badge,
        toggle,
        preset,
    }
}

pub(crate) fn build_edge_community_pick() -> CommunityPick {
    CommunityPick::new(CommunityPickDraft {
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
    .expect("edge community pick should be valid")
}

pub(crate) fn build_tag() -> Tag {
    Tag::new(
        TAG_ID,
        "family-friendly",
        icon("tag:family"),
        localizations("Family"),
    )
    .expect("tag should be valid")
}
