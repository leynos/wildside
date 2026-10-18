//! Dense boundary regression for route-coordinate rounding.

use super::super::{MAX_COORDINATE_MAGNITUDE, round_coordinate_value};

const BOUNDARY_SLICE_ULPS: u64 = 100_000;
const COORDINATE_SCALE: f64 = 100_000.0;

#[test]
fn dense_rounding_slice_near_coordinate_bound() {
    for boundary in [-180.0_f64, 180.0_f64] {
        let boundary_bits = boundary.to_bits();

        for distance in 1..=BOUNDARY_SLICE_ULPS {
            // Decrementing the bit pattern moves inward for both signed
            // boundaries because IEEE-754 orders negative values in reverse.
            let coordinate = f64::from_bits(boundary_bits - distance);
            let rounded = round_coordinate_value(coordinate);
            let original_grid_cell = (coordinate * COORDINATE_SCALE).round();
            let rounded_grid_cell = (rounded * COORDINATE_SCALE).round();
            let rounded_again = round_coordinate_value(rounded);

            assert!(
                rounded.abs() <= MAX_COORDINATE_MAGNITUDE,
                "rounded value {rounded} exceeded the admission bound near {boundary}"
            );
            assert_eq!(
                rounded_grid_cell, original_grid_cell,
                "rounding changed the grid cell near {boundary} at offset {distance} ULPs"
            );
            assert_eq!(
                rounded_again.to_bits(),
                rounded.to_bits(),
                "rounding was not bitwise idempotent near {boundary} at offset {distance} ULPs"
            );
        }
    }
}
