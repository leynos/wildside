//! Properties for coordinate rounding equivalence and grid-cell divergence.

use proptest::prelude::*;
use serde_json::json;

use super::super::super::ROUNDED_COORDINATE_KEYS;
use super::strategies::{
    distinct_integer_pair, divergent_cells, grid_cell_pair, non_numeric_coordinate_pair,
};
use crate::domain::ports::{RouteCacheKey, RouteCacheKeyDerivationError};

const COORDINATE_SCALE: f64 = 100_000.0;

proptest! {
    #[test]
    fn coordinates_in_one_grid_cell_share_a_key(
        (first_coordinate, second_coordinate) in grid_cell_pair(),
        integer_coordinate in -180_i16..=180,
    ) {
        for &field in ROUNDED_COORDINATE_KEYS {
            let first = json!({
                field: first_coordinate,
                "waypoints": [{field: first_coordinate}],
            });
            let second = json!({
                field: second_coordinate,
                "waypoints": [{field: second_coordinate}],
            });
            let first_key = RouteCacheKey::for_route_request(&first);
            let second_key = RouteCacheKey::for_route_request(&second);

            prop_assert!(first_key.is_ok(), "in-range coordinate: {:?}", first_key.as_ref().err());
            prop_assert!(second_key.is_ok(), "in-range coordinate: {:?}", second_key.as_ref().err());
            prop_assert_eq!(first_key.ok(), second_key.ok());

            let integer_payload = json!({
                field: integer_coordinate,
                "waypoints": [{field: integer_coordinate}],
            });
            let float_payload = json!({
                field: integer_coordinate as f64,
                "waypoints": [{field: integer_coordinate as f64}],
            });
            let integer_key = RouteCacheKey::for_route_request(&integer_payload);
            let float_key = RouteCacheKey::for_route_request(&float_payload);

            prop_assert!(integer_key.is_ok(), "integer coordinate: {:?}", integer_key.as_ref().err());
            prop_assert!(float_key.is_ok(), "float coordinate: {:?}", float_key.as_ref().err());
            prop_assert_eq!(integer_key.ok(), float_key.ok());
        }
    }

    #[test]
    fn coordinates_in_distinct_grid_cells_diverge((first_cell, second_cell) in divergent_cells()) {
        let first = json!({"lat": first_cell as f64 / COORDINATE_SCALE});
        let second = json!({"lat": second_cell as f64 / COORDINATE_SCALE});
        let first_key = RouteCacheKey::for_route_request(&first);
        let second_key = RouteCacheKey::for_route_request(&second);

        prop_assert!(first_key.is_ok(), "in-range coordinate: {:?}", first_key.as_ref().err());
        prop_assert!(second_key.is_ok(), "in-range coordinate: {:?}", second_key.as_ref().err());
        prop_assert_ne!(first_key.ok(), second_key.ok());
    }

    #[test]
    fn distinct_integer_coordinates_do_not_collapse(
        (first_integer, second_integer) in distinct_integer_pair(),
    ) {
        let first_key = RouteCacheKey::for_route_request(&serde_json::json!({"lat": first_integer}));
        let second_key = RouteCacheKey::for_route_request(&serde_json::json!({"lat": second_integer}));

        prop_assert!(
            first_key.is_err() || second_key.is_err() || first_key != second_key,
            "distinct integer coordinates must be rejected or derive distinct keys: {first_integer:?}, {second_integer:?} => {first_key:?}, {second_key:?}"
        );
    }

    #[test]
    fn non_numeric_coordinate_values_remain_distinct(
        (first, second) in non_numeric_coordinate_pair(),
    ) {
        let first_key = RouteCacheKey::for_route_request(&first);
        let second_key = RouteCacheKey::for_route_request(&second);

        prop_assert!(first_key.is_ok(), "first non-numeric value: {:?}", first_key.as_ref().err());
        prop_assert!(second_key.is_ok(), "second non-numeric value: {:?}", second_key.as_ref().err());
        prop_assert_ne!(first_key.ok(), second_key.ok());
    }
}

#[test]
fn string_values_under_coordinate_keys_remain_distinct() {
    for &field in ROUNDED_COORDINATE_KEYS {
        let first = json!({field: "north"});
        let second = json!({field: "south"});
        let first_key = RouteCacheKey::for_route_request(&first)
            .expect("a string under a coordinate key is admitted");
        let second_key = RouteCacheKey::for_route_request(&second)
            .expect("a string under a coordinate key is admitted");

        assert_ne!(
            first_key, second_key,
            "material string changes under {field} must remain in the key"
        );
    }
}

#[test]
fn non_numeric_coordinate_values_preserve_material_changes() {
    let cases = [
        (
            "boolean values",
            json!({"lng": true}),
            json!({"lng": false}),
        ),
        (
            "null and string values",
            json!({"lon": null}),
            json!({"lon": "unset"}),
        ),
        (
            "object values",
            json!({"latitude": {"label": "north"}}),
            json!({"latitude": {"label": "south"}}),
        ),
    ];

    for (description, first, second) in cases {
        let first_key = RouteCacheKey::for_route_request(&first)
            .expect("a non-numeric value under a coordinate key is admitted");
        let second_key = RouteCacheKey::for_route_request(&second)
            .expect("a non-numeric value under a coordinate key is admitted");

        assert_ne!(
            first_key, second_key,
            "material changes to {description} must remain in the key"
        );
    }
}

#[test]
fn array_values_reset_coordinate_context() {
    let first = json!({"lat": [181.0]});
    let second = json!({"lat": [181.000_001]});
    let first_key = RouteCacheKey::for_route_request(&first)
        .expect("an array element is not a coordinate leaf");
    let second_key = RouteCacheKey::for_route_request(&second)
        .expect("an array element is not a coordinate leaf");

    assert_ne!(
        first_key, second_key,
        "array elements under lat must not be rounded as coordinate values"
    );
}

#[test]
fn nested_object_restores_coordinate_context_inside_an_array() {
    let payload = json!({"lat": [{"lng": 181.0}]});

    assert!(matches!(
        RouteCacheKey::for_route_request(&payload),
        Err(RouteCacheKeyDerivationError::CoordinateOutOfRange { ref key, .. }) if key == "lng"
    ));
}
