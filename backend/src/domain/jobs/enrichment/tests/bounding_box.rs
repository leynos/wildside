//! Bounding-box validation tests for enrichment jobs.

use pretty_assertions::assert_eq;
use proptest::prelude::*;
use rstest::rstest;

use crate::domain::jobs::{BoundingBox, BoundingBoxError};

#[rstest]
fn bounding_box_accepts_valid_coordinates() {
    let bounding_box = BoundingBox::new(-180.0, -90.0, 180.0, 90.0)
        .expect("world-spanning bounding box should be valid");

    assert_eq!(bounding_box.coords(), [-180.0, -90.0, 180.0, 90.0]);
}

#[derive(Clone)]
struct InvalidBoundingBoxCase {
    min_lng: f64,
    min_lat: f64,
    max_lng: f64,
    max_lat: f64,
    expected: BoundingBoxError,
}

#[rstest]
#[case(InvalidBoundingBoxCase {
    min_lng: f64::NAN,
    min_lat: 0.0,
    max_lng: 1.0,
    max_lat: 1.0,
    expected: BoundingBoxError::NonFinite,
})]
#[case(InvalidBoundingBoxCase {
    min_lng: -181.0,
    min_lat: 0.0,
    max_lng: 1.0,
    max_lat: 1.0,
    expected: BoundingBoxError::LongitudeOutOfRange,
})]
#[case(InvalidBoundingBoxCase {
    min_lng: 0.0,
    min_lat: -91.0,
    max_lng: 1.0,
    max_lat: 1.0,
    expected: BoundingBoxError::LatitudeOutOfRange,
})]
#[case(InvalidBoundingBoxCase {
    min_lng: 0.0,
    min_lat: 0.0,
    max_lng: 0.0,
    max_lat: 1.0,
    expected: BoundingBoxError::AntimeridianWrap,
})]
#[case(InvalidBoundingBoxCase {
    min_lng: 0.0,
    min_lat: 1.0,
    max_lng: 1.0,
    max_lat: 1.0,
    expected: BoundingBoxError::InvertedOrdering,
})]
fn bounding_box_rejects_invalid_coordinates(#[case] case: InvalidBoundingBoxCase) {
    let error = BoundingBox::new(case.min_lng, case.min_lat, case.max_lng, case.max_lat)
        .expect_err("invalid bounding box should be rejected");

    assert_eq!(error, case.expected);
}

proptest! {
    #[test]
    fn non_finite_bounding_box_coordinates_are_rejected(
        coord_index in 0_usize..4,
        non_finite in prop_oneof![Just(f64::NAN), Just(f64::INFINITY), Just(f64::NEG_INFINITY)],
    ) {
        let mut coords = [-1.0, -1.0, 1.0, 1.0];
        coords[coord_index] = non_finite;

        let error = BoundingBox::new(coords[0], coords[1], coords[2], coords[3])
            .expect_err("non-finite coordinates should be rejected");

        prop_assert_eq!(error, BoundingBoxError::NonFinite);
    }

    #[test]
    fn out_of_range_bounding_box_longitudes_are_rejected(
        is_min_lng_invalid in any::<bool>(),
        invalid_lng in prop_oneof![-1_000.0_f64..-180.0, 180.000_001_f64..1_000.0],
    ) {
        let (min_lng, max_lng) = if is_min_lng_invalid {
            (invalid_lng, 1.0)
        } else {
            (-1.0, invalid_lng)
        };

        let error = BoundingBox::new(min_lng, -1.0, max_lng, 1.0)
            .expect_err("out-of-range longitudes should be rejected");

        prop_assert_eq!(error, BoundingBoxError::LongitudeOutOfRange);
    }

    #[test]
    fn out_of_range_bounding_box_latitudes_are_rejected(
        is_min_lat_invalid in any::<bool>(),
        invalid_lat in prop_oneof![-1_000.0_f64..-90.0, 90.000_001_f64..1_000.0],
    ) {
        let (min_lat, max_lat) = if is_min_lat_invalid {
            (invalid_lat, 1.0)
        } else {
            (-1.0, invalid_lat)
        };

        let error = BoundingBox::new(-1.0, min_lat, 1.0, max_lat)
            .expect_err("out-of-range latitudes should be rejected");

        prop_assert_eq!(error, BoundingBoxError::LatitudeOutOfRange);
    }

    #[test]
    fn inverted_bounding_box_longitude_ordering_is_rejected(
        min_lng in -179.0_f64..179.0,
        min_lat in -89.0_f64..89.0,
        lat_span in 0.001_f64..1.0,
    ) {
        let error = BoundingBox::new(min_lng, min_lat, min_lng, min_lat + lat_span)
            .expect_err("equal longitudes should be rejected");

        prop_assert_eq!(error, BoundingBoxError::AntimeridianWrap);
    }

    #[test]
    fn inverted_bounding_box_latitude_ordering_is_rejected(
        min_lng in -179.0_f64..179.0,
        lng_span in 0.001_f64..1.0,
        min_lat in -89.0_f64..90.0,
        inverted_lat_span in 0.0_f64..1.0,
    ) {
        let max_lng = (min_lng + lng_span).min(180.0);
        let max_lat = (min_lat - inverted_lat_span).max(-90.0);

        let error = BoundingBox::new(min_lng, min_lat, max_lng, max_lat)
            .expect_err("inverted latitudes should be rejected");

        prop_assert_eq!(error, BoundingBoxError::InvertedOrdering);
    }
}
