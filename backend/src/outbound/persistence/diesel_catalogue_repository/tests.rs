//! Unit coverage for the catalogue row-conversion helpers.
//!
//! `convert_rows` and `convert_community_pick` decide how a whole result set
//! reports failure, so these cases pin two properties the port relies on: a
//! conversion stops at the first bad row, and every conversion failure reaches
//! the caller as a query error rather than as a silently shorter snapshot.

use std::{collections::BTreeMap, error::Error as StdError};

use uuid::Uuid;

use super::super::json_serializers::{image_asset_to_json, localization_map_to_json};
use super::*;
use crate::domain::{ImageAsset, LocalizationMap, LocalizedStringSet};

type TestResult<T = ()> = Result<T, Box<dyn StdError>>;

/// A converter that rejects `0`, so a failing row is easy to stage.
fn positive(value: u32) -> Result<u32, String> {
    if value == 0 {
        Err("zero is not a valid row".to_owned())
    } else {
        Ok(value)
    }
}

#[test]
fn convert_rows_maps_every_row() -> TestResult {
    let converted = convert_rows(vec![1, 2, 3], positive).expect("every row should convert");

    assert_eq!(
        converted,
        vec![1, 2, 3],
        "conversion should preserve the row order and each converted value"
    );
    Ok(())
}

#[test]
fn convert_rows_reports_the_first_failure_as_a_query_error() {
    let error = convert_rows(vec![1, 0, 3], positive)
        .expect_err("a failing row should abort the conversion");

    assert!(
        matches!(&error, CatalogueRepositoryError::Query { .. }),
        "a conversion failure should surface as a query error, got {error:?}"
    );
    assert!(
        error.to_string().contains("zero is not a valid row"),
        "the row's own message should be preserved, got `{error}`"
    );
}

/// The validated localization map a community pick row carries.
fn localizations() -> TestResult<LocalizationMap> {
    let mut values = BTreeMap::new();
    values.insert(
        "en-GB".to_owned(),
        LocalizedStringSet::new("Curator's pick", None, None),
    );
    Ok(LocalizationMap::new(values)?)
}

/// The validated avatar a community pick row carries.
fn avatar() -> TestResult<ImageAsset> {
    Ok(ImageAsset::new(
        "https://example.test/curator.png",
        "Curator portrait",
    )?)
}

/// A row the conversion should accept, before any case-specific damage.
fn community_pick_row() -> TestResult<CommunityPickRow> {
    Ok(CommunityPickRow {
        id: Uuid::new_v4(),
        route_summary_id: None,
        user_id: None,
        localizations: localization_map_to_json(&localizations()?),
        curator_display_name: "Ada".to_owned(),
        curator_avatar: image_asset_to_json(&avatar()?),
        rating: 4.5,
        distance_metres: 1_200,
        duration_seconds: 900,
        saves: 12,
    })
}

#[test]
fn convert_community_pick_maps_a_present_row() -> TestResult {
    let pick = convert_community_pick(Some(community_pick_row()?))
        .expect("a valid row should convert")
        .expect("a present row should produce a pick");

    assert_eq!(
        pick.curator_display_name(),
        "Ada",
        "the curator's name should survive the conversion"
    );
    Ok(())
}

#[test]
fn convert_community_pick_passes_over_an_absent_row() {
    let converted = convert_community_pick(None).expect("an absent row is not a failure");

    assert!(
        converted.is_none(),
        "a missing community pick should stay absent rather than becoming a default"
    );
}

#[test]
fn convert_community_pick_reports_a_failing_row_as_a_query_error() -> TestResult {
    let row = CommunityPickRow {
        localizations: serde_json::json!({}),
        ..community_pick_row()?
    };

    let error = convert_community_pick(Some(row))
        .expect_err("a row without localizations should be rejected");

    assert!(
        matches!(&error, CatalogueRepositoryError::Query { .. }),
        "an unconvertible row should surface as a query error, got {error:?}"
    );
    assert!(
        error.to_string().contains("localization"),
        "the validation failure should name the offending field, got `{error}`"
    );
    Ok(())
}
