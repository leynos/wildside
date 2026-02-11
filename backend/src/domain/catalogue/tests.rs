//! Unit tests for catalogue domain type construction.

use std::collections::BTreeMap;

use rstest::rstest;
use uuid::Uuid;

use super::*;
use crate::domain::localization::{LocalizationMap, LocalizedStringSet};
use crate::domain::semantic_icon_identifier::SemanticIconIdentifier;

fn localizations() -> LocalizationMap {
    let mut values = BTreeMap::new();
    values.insert(
        "en-GB".to_owned(),
        LocalizedStringSet::new("Scenic route", Some("Scenic".to_owned()), None),
    );
    LocalizationMap::new(values).expect("valid localizations")
}

fn icon_key() -> SemanticIconIdentifier {
    SemanticIconIdentifier::new("category:nature").expect("valid icon key")
}

fn image() -> ImageAsset {
    ImageAsset::new("https://example.test/hero.jpg", "Cliff path").expect("valid image")
}

fn route_summary_draft(slug: Option<String>) -> RouteSummaryDraft {
    RouteSummaryDraft {
        id: Uuid::new_v4(),
        route_id: Uuid::new_v4(),
        category_id: Uuid::new_v4(),
        theme_id: Uuid::new_v4(),
        slug,
        localizations: localizations(),
        hero_image: image(),
        distance_metres: 3_500,
        duration_seconds: 4_200,
        rating: 4.7,
        badge_ids: vec![Uuid::new_v4()],
        difficulty: "moderate".to_owned(),
        interest_theme_ids: vec![Uuid::new_v4()],
    }
}

#[rstest]
fn route_category_rejects_invalid_slug() {
    let result = RouteCategory::new(RouteCategoryDraft {
        id: Uuid::new_v4(),
        slug: "Nature Walk".to_owned(),
        icon_key: icon_key(),
        localizations: localizations(),
        route_count: 10,
    });

    assert!(matches!(
        result,
        Err(CatalogueValidationError::InvalidSlug {
            field: "route_category.slug",
        })
    ));
}

#[rstest]
fn theme_new_accepts_valid_payload() {
    let theme = Theme::new(ThemeDraft {
        id: Uuid::new_v4(),
        slug: "nature".to_owned(),
        icon_key: icon_key(),
        localizations: localizations(),
        image: image(),
        walk_count: 25,
        distance_range_metres: [1_000, 6_000],
        rating: 4.2,
    })
    .expect("valid theme");

    assert_eq!(theme.slug, "nature");
}

#[rstest]
fn theme_rejects_invalid_range() {
    let result = Theme::new(ThemeDraft {
        id: Uuid::new_v4(),
        slug: "nature".to_owned(),
        icon_key: icon_key(),
        localizations: localizations(),
        image: image(),
        walk_count: 25,
        distance_range_metres: [6_000, 1_000],
        rating: 4.2,
    });

    assert!(matches!(
        result,
        Err(CatalogueValidationError::InvalidRange {
            field: "theme.distance_range_metres",
            ..
        })
    ));
}

#[rstest]
fn route_collection_rejects_empty_difficulty() {
    let result = RouteCollection::new(RouteCollectionDraft {
        id: Uuid::new_v4(),
        slug: "coastal-collection".to_owned(),
        icon_key: icon_key(),
        localizations: localizations(),
        lead_image: image(),
        map_preview: image(),
        distance_range_metres: [500, 2_500],
        duration_range_seconds: [1_200, 3_600],
        difficulty: "  ".to_owned(),
        route_ids: vec![Uuid::new_v4()],
    });

    assert!(matches!(
        result,
        Err(CatalogueValidationError::EmptyField {
            field: "route_collection.difficulty",
        })
    ));
}

#[rstest]
#[case(Some("coastal-route".to_owned()), Some("coastal-route"))]
#[case(None, None)]
fn route_summary_accepts_valid_slug_variants(
    #[case] slug_input: Option<String>,
    #[case] expected_slug: Option<&str>,
) {
    let summary =
        RouteSummary::new(route_summary_draft(slug_input)).expect("valid route summary draft");

    assert_eq!(summary.slug.as_deref(), expected_slug);
}

#[rstest]
fn route_summary_rejects_negative_distance() {
    let result = RouteSummary::new(RouteSummaryDraft {
        id: Uuid::new_v4(),
        route_id: Uuid::new_v4(),
        category_id: Uuid::new_v4(),
        theme_id: Uuid::new_v4(),
        slug: None,
        localizations: localizations(),
        hero_image: image(),
        distance_metres: -1,
        duration_seconds: 4_200,
        rating: 4.7,
        badge_ids: vec![],
        difficulty: "easy".to_owned(),
        interest_theme_ids: vec![],
    });

    assert!(matches!(
        result,
        Err(CatalogueValidationError::NegativeValue {
            field: "route_summary.distance_metres",
            ..
        })
    ));
}

#[rstest]
fn trending_highlight_rejects_empty_delta() {
    let result =
        TrendingRouteHighlight::new(Uuid::new_v4(), Uuid::new_v4(), "   ", localizations());

    assert!(matches!(
        result,
        Err(CatalogueValidationError::EmptyField {
            field: "trending_route_highlight.trend_delta",
        })
    ));
}

#[rstest]
fn community_pick_rejects_empty_curator_name() {
    let result = CommunityPick::new(CommunityPickDraft {
        id: Uuid::new_v4(),
        route_summary_id: None,
        user_id: None,
        localizations: localizations(),
        curator_display_name: "  ".to_owned(),
        curator_avatar: image(),
        rating: 4.5,
        distance_metres: 600,
        duration_seconds: 900,
        saves: 24,
    });

    assert!(matches!(
        result,
        Err(CatalogueValidationError::EmptyField {
            field: "community_pick.curator_display_name",
        })
    ));
}

#[rstest]
fn community_pick_accepts_optional_references() {
    let pick = CommunityPick::new(CommunityPickDraft {
        id: Uuid::new_v4(),
        route_summary_id: None,
        user_id: None,
        localizations: localizations(),
        curator_display_name: "Trail Team".to_owned(),
        curator_avatar: image(),
        rating: 4.0,
        distance_metres: 1_250,
        duration_seconds: 2_400,
        saves: 0,
    })
    .expect("valid community pick");

    assert!(pick.route_summary_id.is_none());
    assert!(pick.user_id.is_none());
}
