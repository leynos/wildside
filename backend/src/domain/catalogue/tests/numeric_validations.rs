//! Numeric validation coverage for catalogue domain constructors.

use rstest::rstest;
use uuid::Uuid;

use super::super::*;

#[rstest]
#[case(-0.1)]
#[case(5.1)]
fn theme_rejects_out_of_range_rating(#[case] rating: f32) {
    let result = Theme::new(ThemeDraft {
        id: Uuid::new_v4(),
        slug: "nature".to_owned(),
        icon_key: super::icon_key(),
        localizations: super::localizations(),
        image: super::image(),
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
        icon_key: super::icon_key(),
        localizations: super::localizations(),
        image: super::image(),
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
fn route_summary_rejects_negative_duration_seconds() {
    let mut route_summary_draft = super::route_summary_draft();
    route_summary_draft.duration_seconds = -1;
    let result = RouteSummary::new(route_summary_draft);

    assert!(matches!(
        result,
        Err(CatalogueValidationError::NegativeValue {
            field: "route_summary.duration_seconds",
            ..
        })
    ));
}

#[rstest]
#[case(-0.1)]
#[case(5.1)]
fn route_summary_rejects_out_of_range_rating(#[case] rating: f32) {
    let mut route_summary_draft = super::route_summary_draft();
    route_summary_draft.rating = rating;
    let result = RouteSummary::new(route_summary_draft);

    assert!(matches!(
        result,
        Err(CatalogueValidationError::InvalidRating {
            field: "route_summary.rating",
            ..
        })
    ));
}

#[rstest]
#[case(-0.1)]
#[case(5.1)]
fn community_pick_rejects_out_of_range_rating(#[case] rating: f32) {
    let mut draft = super::community_pick_draft(None, None, "Trail Team");
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
fn community_pick_rejects_negative_duration_seconds() {
    let mut draft = super::community_pick_draft(None, None, "Trail Team");
    draft.duration_seconds = -1;
    let result = CommunityPick::new(draft);

    assert!(matches!(
        result,
        Err(CatalogueValidationError::NegativeValue {
            field: "community_pick.duration_seconds",
            ..
        })
    ));
}

#[rstest]
fn community_pick_rejects_negative_saves() {
    let mut draft = super::community_pick_draft(None, None, "Trail Team");
    draft.saves = -1;
    let result = CommunityPick::new(draft);

    assert!(matches!(
        result,
        Err(CatalogueValidationError::NegativeValue {
            field: "community_pick.saves",
            ..
        })
    ));
}
