//! Properties for full-domain normalization and route-key derivation.

use proptest::prelude::*;
use serde_json::Value;

use super::super::super::{
    MAX_COORDINATE_MAGNITUDE, ROUNDED_COORDINATE_KEYS, normalize_route_request_value,
};
use super::strategies::route_payload;
use crate::domain::ports::{RouteCacheKey, RouteCacheKeyDerivationError};

proptest! {
    #[test]
    fn normalization_is_idempotent(payload in route_payload()) {
        if let Ok(normalized) = normalize_route_request_value(&payload, None) {
            let normalized_again = normalize_route_request_value(&normalized, None);

            prop_assert_eq!(normalized_again, Ok(normalized.clone()));

            let payload_key = RouteCacheKey::for_route_request(&payload);
            let normalized_key = RouteCacheKey::for_route_request(&normalized);

            prop_assert!(payload_key.is_ok(), "generated payload key: {:?}", payload_key.as_ref().err());
            prop_assert!(normalized_key.is_ok(), "normalized payload key: {:?}", normalized_key.as_ref().err());
            prop_assert_eq!(payload_key.ok(), normalized_key.ok());
        }
    }

    #[test]
    fn derived_keys_match_route_v1_format(payload in route_payload()) {
        let should_succeed = coordinate_values_are_admitted(&payload, None);
        let result = RouteCacheKey::for_route_request(&payload);

        prop_assert_eq!(result.is_ok(), should_succeed);
        if !should_succeed {
            prop_assert!(
                matches!(&result, Err(RouteCacheKeyDerivationError::CoordinateOutOfRange { .. })),
                "out-of-range coordinate returns a typed admission error: {result:?}"
            );
        }

        if let Ok(key) = result {
            prop_assert!(
                has_route_v1_digest_format(key.as_str()),
                "derived key has the route:v1 format: {key}"
            );
        }
    }

    #[test]
    fn out_of_range_coordinates_are_rejected(payload in route_payload()) {
        let mut offending = Vec::new();
        collect_out_of_range_coordinates(&payload, None, &mut offending);

        if !offending.is_empty() {
            let result = RouteCacheKey::for_route_request(&payload);

            match &result {
                Err(RouteCacheKeyDerivationError::CoordinateOutOfRange { key, value }) => {
                    let reported_value = value.to_string();
                    prop_assert!(
                        offending.iter().any(|(offending_key, offending_value)| {
                            offending_key == key && offending_value == &reported_value
                        }),
                        "reported coordinate {key}={value} is one of the offending leaves: {offending:?}"
                    );
                }
                _ => prop_assert!(
                    false,
                    "out-of-range coordinates must return CoordinateOutOfRange: {result:?}"
                ),
            }
        }
    }
}

fn collect_out_of_range_coordinates(
    value: &Value,
    current_key: Option<&str>,
    offending: &mut Vec<(String, String)>,
) {
    match value {
        Value::Number(number)
            if current_key.is_some_and(|key| ROUNDED_COORDINATE_KEYS.contains(&key))
                && number
                    .as_f64()
                    .is_some_and(|value| value.abs() > MAX_COORDINATE_MAGNITUDE) =>
        {
            if let Some(key) = current_key {
                offending.push((key.to_owned(), number.to_string()));
            }
        }
        Value::Object(entries) => {
            for (key, child) in entries {
                collect_out_of_range_coordinates(child, Some(key), offending);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_out_of_range_coordinates(item, None, offending);
            }
        }
        _ => {}
    }
}

fn coordinate_values_are_admitted(value: &Value, current_key: Option<&str>) -> bool {
    match value {
        Value::Number(number)
            if current_key.is_some_and(|key| ROUNDED_COORDINATE_KEYS.contains(&key)) =>
        {
            number
                .as_f64()
                .is_some_and(|value| value.abs() <= MAX_COORDINATE_MAGNITUDE)
        }
        Value::Object(entries) => entries
            .iter()
            .all(|(key, child)| coordinate_values_are_admitted(child, Some(key))),
        Value::Array(items) => items
            .iter()
            .all(|item| coordinate_values_are_admitted(item, None)),
        _ => true,
    }
}

fn has_route_v1_digest_format(key: &str) -> bool {
    let Some(digest) = key.strip_prefix("route:v1:") else {
        return false;
    };

    digest.len() == 64
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
