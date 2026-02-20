//! Offline bundle validation and conversion helpers.

use super::{
    OfflineValidationError,
    bundle::{OfflineBundle, OfflineBundleDraft},
    enums::{OfflineBundleKind, OfflineBundleStatus},
};

impl TryFrom<OfflineBundleDraft> for OfflineBundle {
    type Error = OfflineValidationError;

    fn try_from(draft: OfflineBundleDraft) -> Result<Self, Self::Error> {
        let device_id = validate_device_id(&draft.device_id)?;
        validate_progress(draft.progress)?;
        validate_timestamps(draft.created_at, draft.updated_at)?;
        validate_kind_specific(draft.kind, draft.route_id, draft.region_id.clone())?;
        validate_status_progress(draft.status, draft.progress)?;

        Ok(Self {
            id: draft.id,
            owner_user_id: draft.owner_user_id,
            device_id,
            kind: draft.kind,
            route_id: draft.route_id,
            region_id: draft.region_id.map(|value| value.trim().to_owned()),
            bounds: draft.bounds,
            zoom_range: draft.zoom_range,
            estimated_size_bytes: draft.estimated_size_bytes,
            created_at: draft.created_at,
            updated_at: draft.updated_at,
            status: draft.status,
            progress: draft.progress,
        })
    }
}

/// Validates and normalizes the bundle device identifier.
///
/// # Examples
///
/// `validate_device_id("  ios-phone  ")` returns `"ios-phone"`.
fn validate_device_id(device_id: &str) -> Result<String, OfflineValidationError> {
    let normalized = device_id.trim().to_owned();
    if normalized.is_empty() {
        return Err(OfflineValidationError::EmptyDeviceId);
    }
    Ok(normalized)
}

/// Validates that bundle progress is within the inclusive range `[0.0, 1.0]`.
///
/// # Examples
///
/// `validate_progress(0.5)` succeeds, while `validate_progress(1.2)` fails.
fn validate_progress(progress: f32) -> Result<(), OfflineValidationError> {
    if !(0.0..=1.0).contains(&progress) {
        return Err(OfflineValidationError::InvalidProgress { progress });
    }
    Ok(())
}

/// Validates that update timestamps do not precede creation timestamps.
///
/// # Examples
///
/// `validate_timestamps(now, now)` succeeds.
fn validate_timestamps(
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
) -> Result<(), OfflineValidationError> {
    if updated_at < created_at {
        return Err(OfflineValidationError::UpdatedBeforeCreated);
    }
    Ok(())
}

/// Validates route-bundle requirements for route and region identifiers.
///
/// # Examples
///
/// A route bundle with `Some(route_id)` and `None` region id succeeds.
fn validate_route_bundle(
    route_id: Option<uuid::Uuid>,
    region_id: Option<String>,
) -> Result<(), OfflineValidationError> {
    if route_id.is_none() {
        return Err(OfflineValidationError::MissingRouteIdForRouteBundle);
    }
    if region_id.is_some() {
        return Err(OfflineValidationError::UnexpectedRegionIdForRouteBundle);
    }
    Ok(())
}

/// Validates region-bundle requirements for route and region identifiers.
///
/// # Examples
///
/// A region bundle with `None` route id and non-blank region id succeeds.
fn validate_region_bundle(
    route_id: Option<uuid::Uuid>,
    region_id: Option<String>,
) -> Result<(), OfflineValidationError> {
    let region_value = region_id.as_deref().map(str::trim).unwrap_or_default();
    if region_value.is_empty() {
        return Err(OfflineValidationError::MissingRegionIdForRegionBundle);
    }
    if route_id.is_some() {
        return Err(OfflineValidationError::UnexpectedRouteIdForRegionBundle);
    }
    Ok(())
}

/// Dispatches kind-specific validation for route and region identifiers.
///
/// # Examples
///
/// `validate_kind_specific` delegates based on bundle kind.
fn validate_kind_specific(
    kind: OfflineBundleKind,
    route_id: Option<uuid::Uuid>,
    region_id: Option<String>,
) -> Result<(), OfflineValidationError> {
    match kind {
        OfflineBundleKind::Route => validate_route_bundle(route_id, region_id),
        OfflineBundleKind::Region => validate_region_bundle(route_id, region_id),
    }
}

fn validate_status_progress(
    status: OfflineBundleStatus,
    progress: f32,
) -> Result<(), OfflineValidationError> {
    let is_valid_status_progress = is_status_progress_valid(status, progress);

    if is_valid_status_progress {
        Ok(())
    } else {
        Err(OfflineValidationError::InvalidStatusProgress { status, progress })
    }
}

fn is_status_progress_valid(status: OfflineBundleStatus, progress: f32) -> bool {
    match status {
        OfflineBundleStatus::Queued => progress == 0.0,
        OfflineBundleStatus::Downloading => (0.0..1.0).contains(&progress),
        OfflineBundleStatus::Complete => progress == 1.0,
        OfflineBundleStatus::Failed => (0.0..=1.0).contains(&progress),
    }
}
