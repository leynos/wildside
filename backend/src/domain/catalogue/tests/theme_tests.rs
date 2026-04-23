//! Theme-focused catalogue domain tests.

use super::*;
use rstest::rstest;

#[rstest]
fn theme_new_accepts_valid_payload(
    icon_key: TestResult<SemanticIconIdentifier>,
    localizations: TestResult<LocalizationMap>,
    image: TestResult<ImageAsset>,
) -> TestResult {
    let theme = Theme::new(ThemeDraft {
        id: Uuid::new_v4(),
        slug: "nature".to_owned(),
        icon_key: icon_key?,
        localizations: localizations?,
        image: image?,
        walk_count: 25,
        distance_range_metres: [1_000, 6_000],
        rating: 4.2,
    })?;

    assert_eq!(theme.slug(), "nature");
    Ok(())
}

#[rstest]
#[case(
    make_theme_invalid_range,
    "theme.distance_range_metres",
    matches_invalid_range
)]
#[case(
    make_theme_negative_walk_count,
    "theme.walk_count",
    matches_negative_value
)]
fn theme_invalid_inputs(
    #[case] mutator: fn(&mut ThemeDraft),
    #[case] expected_error_field: &str,
    #[case] matcher: fn(&CatalogueValidationError, &str) -> bool,
    default_theme_draft: TestResult<ThemeDraft>,
) -> TestResult {
    let mut draft = default_theme_draft?;
    mutator(&mut draft);
    let result = Theme::new(draft);

    assert!(matches!(result, Err(ref err) if matcher(err, expected_error_field)));
    Ok(())
}

#[rstest]
#[case(-0.1)]
#[case(5.1)]
fn theme_rejects_out_of_range_rating(
    #[case] rating: f32,
    icon_key: TestResult<SemanticIconIdentifier>,
    localizations: TestResult<LocalizationMap>,
    image: TestResult<ImageAsset>,
) -> TestResult {
    let result = Theme::new(ThemeDraft {
        id: Uuid::new_v4(),
        slug: "nature".to_owned(),
        icon_key: icon_key?,
        localizations: localizations?,
        image: image?,
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

fn make_theme_invalid_range(draft: &mut ThemeDraft) {
    draft.walk_count = 25;
    draft.distance_range_metres = [6_000, 1_000];
}

fn make_theme_negative_walk_count(draft: &mut ThemeDraft) {
    draft.walk_count = -1;
}

fn matches_invalid_range(err: &CatalogueValidationError, expected_field: &str) -> bool {
    matches!(
        err,
        CatalogueValidationError::InvalidRange {
            field: actual_field,
            ..
        } if *actual_field == expected_field
    )
}

fn matches_negative_value(err: &CatalogueValidationError, expected_field: &str) -> bool {
    matches!(
        err,
        CatalogueValidationError::NegativeValue {
            field: actual_field,
            ..
        } if *actual_field == expected_field
    )
}
