//! Internal validation helpers for OSM ingestion value-object construction.

use crate::domain::Error;

pub(super) fn is_valid_digest(digest: &str) -> bool {
    digest.len() == 64
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

pub(super) fn validate_bounds(bounds: [f64; 4]) -> Result<(), Error> {
    let [min_lng, min_lat, max_lng, max_lat] = bounds;
    validate_longitude_bounds(min_lng, max_lng)?;
    validate_latitude_bounds(min_lat, max_lat)?;
    validate_bounds_ordering(min_lng, min_lat, max_lng, max_lat)?;
    Ok(())
}

fn validate_coordinate_bounds<F>(
    min: f64,
    max: f64,
    predicate: F,
    error_message: &str,
) -> Result<(), Error>
where
    F: Fn(f64) -> bool,
{
    if !predicate(min) || !predicate(max) {
        return Err(Error::invalid_request(error_message));
    }
    Ok(())
}

fn validate_longitude_bounds(min_lng: f64, max_lng: f64) -> Result<(), Error> {
    validate_coordinate_bounds(
        min_lng,
        max_lng,
        valid_longitude,
        "geofence longitude values must be finite and within [-180, 180]",
    )
}

fn validate_latitude_bounds(min_lat: f64, max_lat: f64) -> Result<(), Error> {
    validate_coordinate_bounds(
        min_lat,
        max_lat,
        valid_latitude,
        "geofence latitude values must be finite and within [-90, 90]",
    )
}

fn validate_bounds_ordering(
    min_lng: f64,
    min_lat: f64,
    max_lng: f64,
    max_lat: f64,
) -> Result<(), Error> {
    if min_lng <= max_lng && min_lat <= max_lat {
        return Ok(());
    }

    Err(Error::invalid_request(
        "geofenceBounds must be ordered as [minLng, minLat, maxLng, maxLat]",
    ))
}

#[rustfmt::skip]
pub(super) fn valid_longitude(value: f64) -> bool { value.is_finite() && (-180.0..=180.0).contains(&value) }

#[rustfmt::skip]
pub(super) fn valid_latitude(value: f64) -> bool { value.is_finite() && (-90.0..=90.0).contains(&value) }
