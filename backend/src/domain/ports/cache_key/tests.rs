//! Validates cache key parsing, canonicalization, and whitespace
//! constraints.
//! TODO: Add property-based tests for canonicalization invariants across
//! generated key ordering, theme arrays, and coordinate rounding cases.
use insta::assert_snapshot;
use serde_json::json;

use crate::domain::idempotency::PayloadHashError;

use super::{
    ROUNDED_COORDINATE_KEYS, RouteCacheKey, RouteCacheKeyDerivationError,
    RouteCacheKeyValidationError, SORTED_ARRAY_KEYS,
};
use rstest::rstest;

#[rstest]
#[case("")]
#[case("   ")]
fn cache_key_rejects_blank(#[case] value: &str) {
    let err = RouteCacheKey::new(value).expect_err("blank keys rejected");
    assert_eq!(err, RouteCacheKeyValidationError::Empty);
}

#[rstest]
#[case(" leading")]
#[case("trailing ")]
fn cache_key_rejects_whitespace_padding(#[case] value: &str) {
    let err = RouteCacheKey::new(value).expect_err("padded key rejected");
    assert_eq!(err, RouteCacheKeyValidationError::ContainsWhitespace);
}

#[rstest]
fn cache_key_accepts_clean_input() {
    let key = RouteCacheKey::new("route:user:1").expect("valid key");
    assert_eq!(key.as_str(), "route:user:1");
    assert_eq!(key.to_string(), "route:user:1");
}

#[test]
fn route_request_key_has_expected_namespace_and_hash_shape() {
    let payload = json!({
        "origin": {"lat": 51.5, "lng": -0.1},
        "destination": {"lat": 48.85661, "lng": 2.35222},
        "preferences": {"interestThemeIds": ["history", "art"]},
    });

    let key = RouteCacheKey::for_route_request(&payload).expect("route key");
    let digest = key
        .as_str()
        .strip_prefix("route:v1:")
        .expect("route namespace");

    assert_eq!(digest.len(), 64);
    assert!(
        digest
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    );
    assert_eq!(digest, digest.to_ascii_lowercase());
}

#[test]
fn route_cache_error_messages_match_snapshots() {
    assert_snapshot!(
        RouteCacheKeyValidationError::Empty.to_string(),
        @"route cache key must not be empty"
    );
    assert_snapshot!(
        RouteCacheKeyValidationError::ContainsWhitespace.to_string(),
        @"route cache key must not contain surrounding whitespace"
    );
    assert_snapshot!(
        RouteCacheKeyValidationError::MalformedDigest.to_string(),
        @"route cache key digest must be a 64-character lowercase hex string"
    );
    assert_snapshot!(
        RouteCacheKeyDerivationError::Hash(PayloadHashError::Serialization {
            message: "not representable as canonical JSON".to_owned(),
        })
        .to_string(),
        @"failed to serialize canonical JSON payload: not representable as canonical JSON"
    );
    assert_snapshot!(
        RouteCacheKeyDerivationError::Validation(
            RouteCacheKeyValidationError::MalformedDigest,
        )
        .to_string(),
        @"route cache key digest must be a 64-character lowercase hex string"
    );
}

#[rstest]
#[case("themes", json!(["history", "art"]), json!(["art", "history"]))]
#[case(
        "themeIds",
        json!(["bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb", "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa"]),
        json!(["aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa", "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"])
    )]
#[case(
        "interestThemeIds",
        json!(["theme-b", "theme-a"]),
        json!(["theme-a", "theme-b"])
    )]
fn route_request_key_sorts_documented_theme_arrays(
    #[case] field_name: &str,
    #[case] first_themes: serde_json::Value,
    #[case] second_themes: serde_json::Value,
) {
    assert!(SORTED_ARRAY_KEYS.contains(&field_name));
    let first = json!({
        "origin": {"lat": 51.5000001, "lng": -0.1000001},
        "destination": {"lat": 48.85661, "lng": 2.35222},
        "preferences": {field_name: first_themes},
    });
    let second = json!({
        "destination": {"lng": 2.35222, "lat": 48.85661},
        "preferences": {field_name: second_themes},
        "origin": {"lng": -0.1, "lat": 51.5},
    });

    let first_key = RouteCacheKey::for_route_request(&first).expect("first route key");
    let second_key = RouteCacheKey::for_route_request(&second).expect("second route key");

    assert_eq!(first_key, second_key);
}

#[rstest]
#[case("lat", 51.5000049, 51.5)]
#[case("lng", -0.1000049, -0.1)]
#[case("latitude", 48.8566141, 48.85661)]
#[case("longitude", 2.3522249, 2.35222)]
#[case("lat", -0.0000049, 0.0000049)]
#[case("lng", -0.0000049, 0.0000049)]
fn route_request_key_rounds_documented_coordinate_fields(
    #[case] field_name: &str,
    #[case] first_coordinate: f64,
    #[case] second_coordinate: f64,
) {
    assert!(ROUNDED_COORDINATE_KEYS.contains(&field_name));
    let first = json!({field_name: first_coordinate});
    let second = json!({field_name: second_coordinate});

    let first_key = RouteCacheKey::for_route_request(&first).expect("first route key");
    let second_key = RouteCacheKey::for_route_request(&second).expect("second route key");

    assert_eq!(first_key, second_key);
}

#[test]
fn route_request_key_changes_for_material_payload_differences() {
    let first = json!({
        "origin": {"lat": 51.5, "lng": -0.1},
        "destination": {"lat": 48.85661, "lng": 2.35222},
        "preferences": {"interestThemeIds": ["art", "history"]},
    });
    let second = json!({
        "origin": {"lat": 51.50002, "lng": -0.1},
        "destination": {"lat": 48.85661, "lng": 2.35222},
        "preferences": {"interestThemeIds": ["art", "history"]},
    });

    let first_key = RouteCacheKey::for_route_request(&first).expect("first route key");
    let second_key = RouteCacheKey::for_route_request(&second).expect("second route key");

    assert_ne!(first_key, second_key);
}

#[test]
fn route_request_key_preserves_non_theme_array_order() {
    let first = json!({
        "origin": {"lat": 51.5, "lng": -0.1},
        "preferences": {"avoid": ["stairs", "crowds"]},
    });
    let second = json!({
        "origin": {"lat": 51.5, "lng": -0.1},
        "preferences": {"avoid": ["crowds", "stairs"]},
    });

    let first_key = RouteCacheKey::for_route_request(&first).expect("first route key");
    let second_key = RouteCacheKey::for_route_request(&second).expect("second route key");

    assert_ne!(first_key, second_key);
}
