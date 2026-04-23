//! Regression coverage for offline bundle domain types.

use std::{error::Error as StdError, io};

use chrono::{Duration, TimeZone, Utc};
use rstest::rstest;
use uuid::Uuid;

use super::{
    BoundingBox, OfflineBundle, OfflineBundleDraft, OfflineBundleKind, OfflineBundleStatus,
    OfflineValidationError, ZoomRange,
};
use crate::domain::UserId;

type TestResult<T = ()> = Result<T, Box<dyn StdError>>;

fn route_bundle_draft() -> TestResult<OfflineBundleDraft> {
    let created_at = Utc
        .with_ymd_and_hms(2026, 2, 20, 8, 0, 0)
        .single()
        .ok_or_else(|| io::Error::other("valid timestamp"))?;

    Ok(OfflineBundleDraft {
        id: Uuid::new_v4(),
        owner_user_id: Some(UserId::random()),
        device_id: "ios-phone".to_owned(),
        kind: OfflineBundleKind::Route,
        route_id: Some(Uuid::new_v4()),
        region_id: None,
        bounds: BoundingBox::new(-3.25, 55.92, -3.10, 56.01)?,
        zoom_range: ZoomRange::new(12, 16)?,
        estimated_size_bytes: 12_000_000,
        created_at,
        updated_at: created_at,
        status: OfflineBundleStatus::Queued,
        progress: 0.0,
    })
}

#[rstest]
fn route_bundle_draft_constructs_bundle() -> TestResult {
    let route_bundle_draft = route_bundle_draft()?;
    let bundle = OfflineBundle::new(route_bundle_draft.clone())?;

    assert_eq!(bundle.kind(), OfflineBundleKind::Route);
    assert_eq!(bundle.route_id(), route_bundle_draft.route_id);
    assert!(bundle.region_id().is_none());
    assert_eq!(bundle.status(), OfflineBundleStatus::Queued);
    assert_eq!(bundle.progress(), 0.0);
    Ok(())
}

#[rstest]
fn route_bundle_draft_trims_device_id() -> TestResult {
    let mut draft = route_bundle_draft()?;
    draft.device_id = "  ios-phone  ".to_owned();

    let bundle = OfflineBundle::new(draft)?;
    assert_eq!(bundle.device_id(), "ios-phone");
    Ok(())
}

#[rstest]
fn route_bundle_draft_rejects_empty_device_id() -> TestResult {
    let mut draft = route_bundle_draft()?;
    draft.device_id = "   ".to_owned();

    let result = OfflineBundle::new(draft);
    assert!(matches!(result, Err(OfflineValidationError::EmptyDeviceId)));
    Ok(())
}

#[rstest]
#[case::route_id_present_for_region(
    Some(Uuid::new_v4()),
    Some("edinburgh-centre".to_owned()),
    OfflineValidationError::UnexpectedRouteIdForRegionBundle
)]
#[case::whitespace_region_id(
    None,
    Some("   ".to_owned()),
    OfflineValidationError::MissingRegionIdForRegionBundle
)]
fn region_bundle_validation_errors(
    #[case] route_id: Option<Uuid>,
    #[case] region_id: Option<String>,
    #[case] expected_error: OfflineValidationError,
) -> TestResult {
    let mut draft = route_bundle_draft()?;
    draft.kind = OfflineBundleKind::Region;
    draft.route_id = route_id;
    draft.region_id = region_id;

    let result = OfflineBundle::new(draft);
    assert!(matches!(result, Err(ref e)
        if std::mem::discriminant(e) == std::mem::discriminant(&expected_error)
    ));
    Ok(())
}

#[rstest]
fn route_bundle_requires_route_id() -> TestResult {
    let mut draft = route_bundle_draft()?;
    draft.route_id = None;

    let result = OfflineBundle::new(draft);
    assert!(matches!(
        result,
        Err(OfflineValidationError::MissingRouteIdForRouteBundle)
    ));
    Ok(())
}

#[rstest]
#[case(OfflineBundleStatus::Queued, 0.0)]
#[case(OfflineBundleStatus::Downloading, 0.5)]
#[case(OfflineBundleStatus::Complete, 1.0)]
#[case(OfflineBundleStatus::Failed, 0.25)]
fn valid_status_progress_pairs_construct(
    #[case] status: OfflineBundleStatus,
    #[case] progress: f32,
) -> TestResult {
    let mut draft = route_bundle_draft()?;
    draft.status = status;
    draft.progress = progress;

    let bundle = OfflineBundle::new(draft)?;
    assert_eq!(bundle.status(), status);
    assert_eq!(bundle.progress(), progress);
    Ok(())
}

#[rstest]
#[case(OfflineBundleStatus::Queued, 0.4)]
#[case(OfflineBundleStatus::Downloading, 1.0)]
#[case(OfflineBundleStatus::Complete, 0.8)]
fn invalid_status_progress_pairs_rejected(
    #[case] status: OfflineBundleStatus,
    #[case] progress: f32,
) -> TestResult {
    let mut draft = route_bundle_draft()?;
    draft.status = status;
    draft.progress = progress;

    let result = OfflineBundle::new(draft);
    assert!(matches!(
        result,
        Err(OfflineValidationError::InvalidStatusProgress { .. })
    ));
    Ok(())
}

#[rstest]
fn invalid_progress_range_rejected() -> TestResult {
    let mut draft = route_bundle_draft()?;
    draft.progress = 1.1;

    let result = OfflineBundle::new(draft);
    assert!(matches!(
        result,
        Err(OfflineValidationError::InvalidProgress { .. })
    ));
    Ok(())
}

#[rstest]
fn updated_at_must_not_precede_created_at() -> TestResult {
    let mut draft = route_bundle_draft()?;
    draft.updated_at = draft.created_at - Duration::seconds(1);

    let result = OfflineBundle::new(draft);
    assert!(matches!(
        result,
        Err(OfflineValidationError::UpdatedBeforeCreated)
    ));
    Ok(())
}

#[rstest]
fn bounding_box_rejects_invalid_coordinates() {
    let result = BoundingBox::new(-190.0, 40.0, -3.0, 50.0);
    assert!(matches!(
        result,
        Err(OfflineValidationError::InvalidBounds {
            field: "min_lng",
            ..
        })
    ));
}

#[rstest]
fn bounding_box_rejects_min_greater_than_max() {
    let result = BoundingBox::new(-3.0, 56.0, -3.2, 55.9);
    assert!(matches!(
        result,
        Err(OfflineValidationError::InvalidBoundsOrder)
    ));
}

#[rstest]
fn zoom_range_rejects_min_greater_than_max() {
    let result = ZoomRange::new(16, 12);
    assert!(matches!(
        result,
        Err(OfflineValidationError::InvalidZoomRange {
            min_zoom: 16,
            max_zoom: 12
        })
    ));
}

#[rstest]
#[case("region", OfflineBundleKind::Region)]
#[case("route", OfflineBundleKind::Route)]
fn bundle_kind_round_trips_from_str(#[case] value: &str, #[case] expected: OfflineBundleKind) {
    let parsed = value.parse::<OfflineBundleKind>().expect("valid kind");
    assert_eq!(parsed, expected);
    assert_eq!(parsed.as_str(), value);
}

#[rstest]
#[case("queued", OfflineBundleStatus::Queued)]
#[case("downloading", OfflineBundleStatus::Downloading)]
#[case("complete", OfflineBundleStatus::Complete)]
#[case("failed", OfflineBundleStatus::Failed)]
fn bundle_status_round_trips_from_str(#[case] value: &str, #[case] expected: OfflineBundleStatus) {
    let parsed = value.parse::<OfflineBundleStatus>().expect("valid status");
    assert_eq!(parsed, expected);
    assert_eq!(parsed.as_str(), value);
}
