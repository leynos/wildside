//! Unit tests for catalogue domain type construction.

use std::{collections::BTreeMap, error::Error as StdError};

use rstest::rstest;
use uuid::Uuid;

use super::*;
use crate::domain::localization::{LocalizationMap, LocalizedStringSet};
use crate::domain::semantic_icon_identifier::SemanticIconIdentifier;

type TestResult<T = ()> = Result<T, Box<dyn StdError>>;

fn localizations() -> TestResult<LocalizationMap> {
    let mut values = BTreeMap::new();
    values.insert(
        "en-GB".to_owned(),
        LocalizedStringSet::new("Scenic route", Some("Scenic".to_owned()), None),
    );
    Ok(LocalizationMap::new(values)?)
}

fn icon_key() -> TestResult<SemanticIconIdentifier> {
    Ok(SemanticIconIdentifier::new("category:nature")?)
}

fn image() -> TestResult<ImageAsset> {
    Ok(ImageAsset::new(
        "https://example.test/hero.jpg",
        "Cliff path",
    )?)
}

fn valid_route_summary_draft() -> TestResult<RouteSummaryDraft> {
    Ok(RouteSummaryDraft {
        id: Uuid::new_v4(),
        route_id: Uuid::new_v4(),
        category_id: Uuid::new_v4(),
        theme_id: Uuid::new_v4(),
        slug: Some("coastal-route".to_owned()),
        localizations: localizations()?,
        hero_image: image()?,
        distance_metres: 3_500,
        duration_seconds: 4_200,
        rating: 4.7,
        badge_ids: vec![Uuid::new_v4()],
        difficulty: "moderate".to_owned(),
        interest_theme_ids: vec![Uuid::new_v4()],
    })
}

#[rstest]
fn image_asset_new_creates_valid_asset() -> TestResult {
    let asset = ImageAsset::new("https://example.test/promo.jpg", "Rocky coast")?;

    assert_eq!(asset.url, "https://example.test/promo.jpg");
    assert_eq!(asset.alt, "Rocky coast");
    Ok(())
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

fn route_summary_draft() -> TestResult<RouteSummaryDraft> {
    valid_route_summary_draft()
}

fn community_pick_draft(
    route_summary_id: Option<Uuid>,
    user_id: Option<Uuid>,
    curator_display_name: impl Into<String>,
) -> TestResult<CommunityPickDraft> {
    Ok(CommunityPickDraft {
        id: Uuid::new_v4(),
        route_summary_id,
        user_id,
        localizations: localizations()?,
        curator_display_name: curator_display_name.into(),
        curator_avatar: image()?,
        rating: 4.0,
        distance_metres: 1_250,
        duration_seconds: 2_400,
        saves: 0,
    })
}

#[rstest]
fn route_category_rejects_invalid_slug() -> TestResult {
    let result = RouteCategory::new(RouteCategoryDraft {
        id: Uuid::new_v4(),
        slug: "Nature Walk".to_owned(),
        icon_key: icon_key()?,
        localizations: localizations()?,
        route_count: 10,
    });

    assert!(matches!(
        result,
        Err(CatalogueValidationError::InvalidSlug {
            field: "route_category.slug",
        })
    ));
    Ok(())
}

#[rstest]
fn theme_new_accepts_valid_payload() -> TestResult {
    let theme = Theme::new(ThemeDraft {
        id: Uuid::new_v4(),
        slug: "nature".to_owned(),
        icon_key: icon_key()?,
        localizations: localizations()?,
        image: image()?,
        walk_count: 25,
        distance_range_metres: [1_000, 6_000],
        rating: 4.2,
    })?;

    assert_eq!(theme.slug(), "nature");
    Ok(())
}

fn default_theme_draft() -> TestResult<ThemeDraft> {
    Ok(ThemeDraft {
        id: Uuid::new_v4(),
        slug: "nature".to_owned(),
        icon_key: icon_key()?,
        localizations: localizations()?,
        image: image()?,
        walk_count: 1,
        distance_range_metres: [1_000, 6_000],
        rating: 4.2,
    })
}

#[rstest]
fn theme_rejects_invalid_range() -> TestResult {
    let mut draft = default_theme_draft()?;
    draft.walk_count = 25;
    draft.distance_range_metres = [6_000, 1_000];
    let result = Theme::new(draft);

    assert!(matches!(
        result,
        Err(CatalogueValidationError::InvalidRange {
            field: "theme.distance_range_metres",
            ..
        })
    ));
    Ok(())
}

#[rstest]
fn route_collection_rejects_empty_difficulty() -> TestResult {
    let result = RouteCollection::new(RouteCollectionDraft {
        id: Uuid::new_v4(),
        slug: "coastal-collection".to_owned(),
        icon_key: icon_key()?,
        localizations: localizations()?,
        lead_image: image()?,
        map_preview: image()?,
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
    Ok(())
}

#[rstest]
fn route_summary_new_accepts_valid_payload() -> TestResult {
    let summary = RouteSummary::new(valid_route_summary_draft()?)?;

    assert_eq!(summary.slug(), Some("coastal-route"));
    Ok(())
}

#[rstest]
fn route_summary_allows_missing_slug() -> TestResult {
    let summary = RouteSummary::new(RouteSummaryDraft {
        slug: None,
        distance_metres: 2_100,
        duration_seconds: 3_000,
        rating: 4.2,
        badge_ids: vec![Uuid::new_v4(), Uuid::new_v4()],
        difficulty: "easy".to_owned(),
        interest_theme_ids: vec![Uuid::new_v4(), Uuid::new_v4()],
        ..valid_route_summary_draft()?
    })?;

    assert_eq!(summary.slug(), None);
    Ok(())
}

#[rstest]
fn trending_highlight_rejects_empty_delta() -> TestResult {
    let result =
        TrendingRouteHighlight::new(Uuid::new_v4(), Uuid::new_v4(), "   ", localizations()?);

    assert!(matches!(
        result,
        Err(CatalogueValidationError::EmptyField {
            field: "trending_route_highlight.trend_delta",
        })
    ));
    Ok(())
}

#[rstest]
fn community_pick_rejects_empty_curator_name() -> TestResult {
    let result = CommunityPick::new(community_pick_draft(None, None, "  ")?);

    assert!(matches!(
        result,
        Err(CatalogueValidationError::EmptyField {
            field: "community_pick.curator_display_name",
        })
    ));
    Ok(())
}

#[rstest]
fn community_pick_accepts_optional_references() -> TestResult {
    let pick = CommunityPick::new(community_pick_draft(None, None, "Trail Team")?)?;

    assert!(pick.route_summary_id().is_none());
    assert!(pick.user_id().is_none());
    Ok(())
}

#[rstest]
#[case(-0.1)]
#[case(5.1)]
fn theme_rejects_out_of_range_rating(#[case] rating: f32) -> TestResult {
    let result = Theme::new(ThemeDraft {
        id: Uuid::new_v4(),
        slug: "nature".to_owned(),
        icon_key: icon_key()?,
        localizations: localizations()?,
        image: image()?,
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
    Ok(())
}

#[rstest]
fn theme_rejects_negative_walk_count() -> TestResult {
    let mut draft = default_theme_draft()?;
    draft.walk_count = -1;
    let result = Theme::new(draft);

    assert!(matches!(
        result,
        Err(CatalogueValidationError::NegativeValue {
            field: "theme.walk_count",
            ..
        })
    ));
    Ok(())
}

#[rstest]
#[case(-0.1)]
#[case(5.1)]
fn route_summary_rejects_out_of_range_rating(#[case] rating: f32) -> TestResult {
    let mut draft = route_summary_draft()?;
    draft.rating = rating;
    let result = RouteSummary::new(draft);

    assert!(matches!(
        result,
        Err(CatalogueValidationError::InvalidRating {
            field: "route_summary.rating",
            ..
        })
    ));
    Ok(())
}

#[rstest]
#[case("distance_metres", "route_summary.distance_metres")]
#[case("duration_seconds", "route_summary.duration_seconds")]
fn route_summary_rejects_negative_numeric_fields(
    #[case] field: &str,
    #[case] expected_error_field: &str,
) -> TestResult {
    let mut draft = route_summary_draft()?;
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
    Ok(())
}

#[rstest]
#[case(-0.1)]
#[case(5.1)]
fn community_pick_rejects_out_of_range_rating(#[case] rating: f32) -> TestResult {
    let mut draft = community_pick_draft(None, None, "Trail Team")?;
    draft.rating = rating;
    let result = CommunityPick::new(draft);

    assert!(matches!(
        result,
        Err(CatalogueValidationError::InvalidRating {
            field: "community_pick.rating",
            ..
        })
    ));
    Ok(())
}

#[rstest]
#[case("duration_seconds", "community_pick.duration_seconds")]
#[case("saves", "community_pick.saves")]
fn community_pick_rejects_negative_numeric_fields(
    #[case] field: &str,
    #[case] expected_error_field: &str,
) -> TestResult {
    let mut draft = community_pick_draft(None, None, "Trail Team")?;
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
    Ok(())
}
