//! Unit tests for catalogue domain type construction.

use std::collections::BTreeMap;

use rstest::{fixture, rstest};
use uuid::Uuid;

use super::*;
use crate::domain::localization::{LocalizationMap, LocalizedStringSet};
use crate::domain::semantic_icon_identifier::SemanticIconIdentifier;

#[fixture]
fn localizations() -> LocalizationMap {
    let mut values = BTreeMap::new();
    values.insert(
        "en-GB".to_owned(),
        LocalizedStringSet::new("Scenic route", Some("Scenic".to_owned()), None),
    );
    LocalizationMap::new(values).expect("valid localizations")
}

#[fixture]
fn icon_key() -> SemanticIconIdentifier {
    SemanticIconIdentifier::new("category:nature").expect("valid icon key")
}

#[fixture]
fn image() -> ImageAsset {
    ImageAsset::new("https://example.test/hero.jpg", "Cliff path").expect("valid image")
}

fn valid_route_summary_draft() -> RouteSummaryDraft {
    RouteSummaryDraft {
        id: Uuid::new_v4(),
        route_id: Uuid::new_v4(),
        category_id: Uuid::new_v4(),
        theme_id: Uuid::new_v4(),
        slug: Some("coastal-route".to_owned()),
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
fn image_asset_new_creates_valid_asset() {
    let asset = ImageAsset::new("https://example.test/promo.jpg", "Rocky coast")
        .expect("image asset should be valid");

    assert_eq!(asset.url, "https://example.test/promo.jpg");
    assert_eq!(asset.alt, "Rocky coast");
}

#[rstest]
#[case("")]
#[case("   ")]
fn image_asset_new_rejects_empty_or_whitespace_url(#[case] url: &str) {
    let result = ImageAsset::new(url, "Alt text");

    assert!(matches!(
        result,
        Err(CatalogueValidationError::EmptyField { field: "image.url" })
    ));
}

#[rstest]
#[case("")]
#[case("   ")]
fn image_asset_new_rejects_empty_or_whitespace_alt(#[case] alt: &str) {
    let result = ImageAsset::new("https://example.test/promo.jpg", alt);

    assert!(matches!(
        result,
        Err(CatalogueValidationError::EmptyField { field: "image.alt" })
    ));
}

#[fixture]
fn route_summary_draft() -> RouteSummaryDraft {
    valid_route_summary_draft()
}

fn community_pick_draft(
    route_summary_id: Option<Uuid>,
    user_id: Option<Uuid>,
    curator_display_name: impl Into<String>,
) -> CommunityPickDraft {
    CommunityPickDraft {
        id: Uuid::new_v4(),
        route_summary_id,
        user_id,
        localizations: localizations(),
        curator_display_name: curator_display_name.into(),
        curator_avatar: image(),
        rating: 4.0,
        distance_metres: 1_250,
        duration_seconds: 2_400,
        saves: 0,
    }
}

#[rstest]
fn route_category_rejects_invalid_slug(
    icon_key: SemanticIconIdentifier,
    localizations: LocalizationMap,
) {
    let result = RouteCategory::new(RouteCategoryDraft {
        id: Uuid::new_v4(),
        slug: "Nature Walk".to_owned(),
        icon_key,
        localizations,
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
fn theme_new_accepts_valid_payload(
    icon_key: SemanticIconIdentifier,
    localizations: LocalizationMap,
    image: ImageAsset,
) {
    let theme = Theme::new(ThemeDraft {
        id: Uuid::new_v4(),
        slug: "nature".to_owned(),
        icon_key,
        localizations,
        image,
        walk_count: 25,
        distance_range_metres: [1_000, 6_000],
        rating: 4.2,
    })
    .expect("valid theme");

    assert_eq!(theme.slug(), "nature");
}

#[rstest]
fn theme_rejects_invalid_range(
    icon_key: SemanticIconIdentifier,
    localizations: LocalizationMap,
    image: ImageAsset,
) {
    let result = Theme::new(ThemeDraft {
        id: Uuid::new_v4(),
        slug: "nature".to_owned(),
        icon_key,
        localizations,
        image,
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
fn route_collection_rejects_empty_difficulty(
    icon_key: SemanticIconIdentifier,
    localizations: LocalizationMap,
    image: ImageAsset,
) {
    let result = RouteCollection::new(RouteCollectionDraft {
        id: Uuid::new_v4(),
        slug: "coastal-collection".to_owned(),
        icon_key,
        localizations,
        lead_image: image.clone(),
        map_preview: image,
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
fn route_summary_new_accepts_valid_payload() {
    let summary =
        RouteSummary::new(valid_route_summary_draft()).expect("valid route summary draft");

    assert_eq!(summary.slug(), Some("coastal-route"));
}

#[rstest]
fn route_summary_allows_missing_slug() {
    let summary = RouteSummary::new(RouteSummaryDraft {
        slug: None,
        distance_metres: 2_100,
        duration_seconds: 3_000,
        rating: 4.2,
        badge_ids: vec![Uuid::new_v4(), Uuid::new_v4()],
        difficulty: "easy".to_owned(),
        interest_theme_ids: vec![Uuid::new_v4(), Uuid::new_v4()],
        ..valid_route_summary_draft()
    })
    .expect("valid route summary draft");

    assert_eq!(summary.slug(), None);
}

#[rstest]
fn trending_highlight_rejects_empty_delta(localizations: LocalizationMap) {
    let result = TrendingRouteHighlight::new(Uuid::new_v4(), Uuid::new_v4(), "   ", localizations);

    assert!(matches!(
        result,
        Err(CatalogueValidationError::EmptyField {
            field: "trending_route_highlight.trend_delta",
        })
    ));
}

#[rstest]
fn community_pick_rejects_empty_curator_name() {
    let result = CommunityPick::new(community_pick_draft(None, None, "  "));

    assert!(matches!(
        result,
        Err(CatalogueValidationError::EmptyField {
            field: "community_pick.curator_display_name",
        })
    ));
}

#[rstest]
fn community_pick_accepts_optional_references() {
    let pick = CommunityPick::new(community_pick_draft(None, None, "Trail Team"))
        .expect("valid community pick");

    assert!(pick.route_summary_id().is_none());
    assert!(pick.user_id().is_none());
}

#[rstest]
#[case(-0.1)]
#[case(5.1)]
fn theme_rejects_out_of_range_rating(#[case] rating: f32) {
    let result = Theme::new(ThemeDraft {
        id: Uuid::new_v4(),
        slug: "nature".to_owned(),
        icon_key: icon_key(),
        localizations: localizations(),
        image: image(),
        walk_count: 25,
        distance_range_metres: [1_000, 6_000],
        rating,
    });

    assert!(matches!(
        result,
        Err(CatalogueValidationError::InvalidRating {
            field: "theme.rating",
            ..
        })
    ));
}

#[rstest]
fn theme_rejects_negative_walk_count() {
    let result = Theme::new(ThemeDraft {
        id: Uuid::new_v4(),
        slug: "nature".to_owned(),
        icon_key: icon_key(),
        localizations: localizations(),
        image: image(),
        walk_count: -1,
        distance_range_metres: [1_000, 6_000],
        rating: 4.2,
    });

    assert!(matches!(
        result,
        Err(CatalogueValidationError::NegativeValue {
            field: "theme.walk_count",
            ..
        })
    ));
}

#[rstest]
#[case(-0.1)]
#[case(5.1)]
fn route_summary_rejects_out_of_range_rating(#[case] rating: f32) {
    let mut draft = route_summary_draft();
    draft.rating = rating;
    let result = RouteSummary::new(draft);

    assert!(matches!(
        result,
        Err(CatalogueValidationError::InvalidRating {
            field: "route_summary.rating",
            ..
        })
    ));
}

#[rstest]
#[case("distance_metres", "route_summary.distance_metres")]
#[case("duration_seconds", "route_summary.duration_seconds")]
fn route_summary_rejects_negative_numeric_fields(
    #[case] field: &str,
    #[case] expected_error_field: &str,
) {
    let mut draft = route_summary_draft();
    match field {
        "distance_metres" => draft.distance_metres = -1,
        "duration_seconds" => draft.duration_seconds = -1,
        _ => unreachable!("unsupported route summary field"),
    }
    let result = RouteSummary::new(draft);

    assert!(matches!(
        result,
        Err(CatalogueValidationError::NegativeValue {
            field: actual_field,
            ..
        }) if actual_field == expected_error_field
    ));
}

#[rstest]
#[case(-0.1)]
#[case(5.1)]
fn community_pick_rejects_out_of_range_rating(#[case] rating: f32) {
    let mut draft = community_pick_draft(None, None, "Trail Team");
    draft.rating = rating;
    let result = CommunityPick::new(draft);

    assert!(matches!(
        result,
        Err(CatalogueValidationError::InvalidRating {
            field: "community_pick.rating",
            ..
        })
    ));
}

#[rstest]
#[case("duration_seconds", "community_pick.duration_seconds")]
#[case("saves", "community_pick.saves")]
fn community_pick_rejects_negative_numeric_fields(
    #[case] field: &str,
    #[case] expected_error_field: &str,
) {
    let mut draft = community_pick_draft(None, None, "Trail Team");
    match field {
        "duration_seconds" => draft.duration_seconds = -1,
        "saves" => draft.saves = -1,
        _ => unreachable!("unsupported community pick field"),
    }
    let result = CommunityPick::new(draft);

    assert!(matches!(
        result,
        Err(CatalogueValidationError::NegativeValue {
            field: actual_field,
            ..
        }) if actual_field == expected_error_field
    ));
}
