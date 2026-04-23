//! Theme-focused catalogue domain tests.

use super::*;
use rstest::rstest;

#[rstest]
fn theme_new_accepts_valid_payload(default_theme_draft: TestResult<ThemeDraft>) -> TestResult {
    let default_theme_draft = default_theme_draft?;
    let theme = Theme::new(ThemeDraft {
        walk_count: 25,
        ..default_theme_draft.clone()
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
    default_theme_draft: TestResult<ThemeDraft>,
) -> TestResult {
    let default_theme_draft = default_theme_draft?;
    let result = Theme::new(ThemeDraft {
        rating,
        walk_count: 25,
        ..default_theme_draft.clone()
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
