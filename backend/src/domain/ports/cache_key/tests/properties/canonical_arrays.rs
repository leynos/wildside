//! Properties for arrays whose order is canonical or material.

use proptest::prelude::*;
use serde_json::json;

use super::super::super::SORTED_ARRAY_KEYS;
use super::strategies::{rotate_by_one, theme_array};
use crate::domain::ports::RouteCacheKey;

proptest! {
    #[test]
    fn theme_permutations_share_a_key(themes in theme_array()) {
        let mut sorted_themes = themes.clone();
        sorted_themes.sort();

        for &field in SORTED_ARRAY_KEYS {
            let first = json!({
                field: themes,
                "preferences": {field: themes.clone()},
            });
            let permuted = json!({
                field: sorted_themes,
                "preferences": {field: sorted_themes.clone()},
            });
            let first_key = RouteCacheKey::for_route_request(&first);
            let permuted_key = RouteCacheKey::for_route_request(&permuted);

            prop_assert!(first_key.is_ok(), "valid theme payload: {:?}", first_key.as_ref().err());
            prop_assert!(permuted_key.is_ok(), "valid theme payload: {:?}", permuted_key.as_ref().err());
            prop_assert_eq!(first_key.ok(), permuted_key.ok());
        }
    }

    #[test]
    fn non_theme_array_order_is_material(length in 2_usize..=8) {
        let values: Vec<String> = (0..length).map(|index| format!("item-{index}")).collect();
        let rotated = rotate_by_one(&values);
        let first = json!({"avoid": values});
        let second = json!({"avoid": rotated});
        let first_key = RouteCacheKey::for_route_request(&first);
        let second_key = RouteCacheKey::for_route_request(&second);

        prop_assert!(first_key.is_ok(), "valid array payload: {:?}", first_key.as_ref().err());
        prop_assert!(second_key.is_ok(), "valid array payload: {:?}", second_key.as_ref().err());
        prop_assert_ne!(first_key.ok(), second_key.ok());
    }

    #[test]
    fn non_string_theme_array_order_is_material(length in 2_usize..=8) {
        let mut values: Vec<serde_json::Value> = (0..length)
            .map(|index| json!(format!("item-{index}")))
            .collect();
        values.push(json!(false));
        let rotated = rotate_by_one(&values);
        let first = json!({"themes": values});
        let second = json!({"themes": rotated});
        let first_key = RouteCacheKey::for_route_request(&first);
        let second_key = RouteCacheKey::for_route_request(&second);

        prop_assert!(first_key.is_ok(), "valid mixed theme payload: {:?}", first_key.as_ref().err());
        prop_assert!(second_key.is_ok(), "valid mixed theme payload: {:?}", second_key.as_ref().err());
        prop_assert_ne!(first_key.ok(), second_key.ok());
    }
}
