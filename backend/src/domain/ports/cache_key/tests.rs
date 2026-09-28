//! Validates cache key parsing, canonicalization, and whitespace
//! constraints.
use insta::assert_snapshot;
use serde_json::json;

use crate::domain::idempotency::PayloadHashError;

use super::{
    ROUNDED_COORDINATE_KEYS, RouteCacheKey, RouteCacheKeyDerivationError,
    RouteCacheKeyValidationError, SORTED_ARRAY_KEYS, is_lowercase_hex_digest,
};
use rstest::rstest;

mod properties;

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
fn route_cache_key_as_ref_exposes_underlying_key() {
    let key = RouteCacheKey::new("route:user:1").expect("valid key");
    let underlying_key: &str = key.as_ref();

    assert_eq!(underlying_key, "route:user:1");
}

#[test]
fn route_digest_validation_requires_lowercase_hex() {
    let lowercase_hex = "a".repeat(64);
    let uppercase_hex = "A".repeat(64);
    let invalid_character = format!("{}g", "a".repeat(63));
    let short_digest = "a".repeat(63);

    assert!(
        is_lowercase_hex_digest(&lowercase_hex),
        "64-character lowercase hexadecimal digests should be accepted"
    );
    assert!(
        !is_lowercase_hex_digest(&uppercase_hex),
        "uppercase hexadecimal characters should be rejected"
    );
    assert!(
        !is_lowercase_hex_digest(&invalid_character),
        "non-hexadecimal characters should be rejected"
    );
    assert!(
        !is_lowercase_hex_digest(&short_digest),
        "digests shorter than 64 characters should be rejected"
    );
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
fn route_request_key_matches_known_sha256_digest() {
    // Independent digest of the compact normalized JSON, computed with:
    // `printf '%s' '{"origin":{"lat":51.5,"lng":-0.1},"preferences":{"interestThemeIds":["art","history"]}}' | sha256sum`
    let payload = json!({
        "origin": {"lat": 51.5, "lng": -0.1},
        "preferences": {"interestThemeIds": ["history", "art"]},
    });

    let key = RouteCacheKey::for_route_request(&payload).expect("route key");

    assert_eq!(
        key.as_str(),
        "route:v1:064a4c57f3b53c1461a025298f66a1393b7b3a0c19cfe19c5297c063c7d06be9"
    );
}

#[test]
fn negative_zero_coordinate_collapses_to_zero() {
    let negative_zero = RouteCacheKey::for_route_request(&json!({"lat": -0.0}))
        .expect("negative-zero coordinate key");
    let positive_zero = RouteCacheKey::for_route_request(&json!({"lat": 0.0}))
        .expect("positive-zero coordinate key");

    assert_eq!(negative_zero, positive_zero);
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
    assert_snapshot!(
        RouteCacheKeyDerivationError::CoordinateOutOfRange {
            key: "lat".to_owned(),
            value: 181.into(),
        }
        .to_string(),
        @"coordinate field 'lat' must be between -180 and 180; got 181"
    );
    assert_snapshot!(
        RouteCacheKeyDerivationError::RoundedCoordinateNotRepresentable {
            key: "lat".to_owned(),
            value: 181.into(),
        }
        .to_string(),
        @"rounded coordinate in field 'lat' is not a finite JSON number: 181"
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

#[rstest]
#[case(1.8e303)]
#[case(1.9e303)]
fn route_request_rejects_rounding_overflow_coordinate(#[case] coordinate: f64) {
    let payload = json!({"lat": coordinate});
    let result = RouteCacheKey::for_route_request(&payload);

    assert!(
        matches!(
            &result,
            Err(RouteCacheKeyDerivationError::CoordinateOutOfRange { key, .. }) if key == "lat"
        ),
        "expected admission rejection for {coordinate}; got {result:?}"
    );
}

#[rstest]
#[case(-42_389_709_114.565025)]
fn large_idempotence_counterexample_is_rejected(#[case] coordinate: f64) {
    let payload = json!({"lat": coordinate});
    let result = RouteCacheKey::for_route_request(&payload);

    assert!(
        matches!(
            &result,
            Err(RouteCacheKeyDerivationError::CoordinateOutOfRange { key, .. }) if key == "lat"
        ),
        "expected idempotence counterexample to be rejected; got {result:?}"
    );
}

#[rstest]
#[case(1_514_566_008_529_973_632_i64, 1_514_566_008_529_973_633_i64)]
fn route_request_rejects_or_distinguishes_colliding_integer_coordinates(
    #[case] first_coordinate: i64,
    #[case] second_coordinate: i64,
) {
    let first_payload = json!({"lat": first_coordinate});
    let second_payload = json!({"lat": second_coordinate});
    let first_key = RouteCacheKey::for_route_request(&first_payload);
    let second_key = RouteCacheKey::for_route_request(&second_payload);

    let first_error = first_key.expect_err("first large integer coordinate rejected");
    let second_error = second_key.expect_err("second large integer coordinate rejected");

    assert_eq!(
        first_error,
        RouteCacheKeyDerivationError::CoordinateOutOfRange {
            key: "lat".to_owned(),
            value: first_coordinate.into(),
        }
    );
    assert_eq!(
        second_error,
        RouteCacheKeyDerivationError::CoordinateOutOfRange {
            key: "lat".to_owned(),
            value: second_coordinate.into(),
        }
    );
}

#[rstest]
#[case(json!(180), true)]
#[case(json!(-180), true)]
#[case(json!(180.0), true)]
#[case(json!(180.000001), false)]
#[case(json!(-180.000001), false)]
#[case(json!(i64::MAX), false)]
#[case(json!(u64::MAX), false)]
#[case(json!(1.0e308), false)]
#[case(json!(f64::from_bits(1)), true)]
fn coordinate_bound_is_inclusive(#[case] coordinate: serde_json::Value, #[case] is_admitted: bool) {
    let payload = json!({"lat": coordinate.clone()});
    let result = RouteCacheKey::for_route_request(&payload);

    assert_eq!(result.is_ok(), is_admitted, "coordinate: {coordinate}");
    if !is_admitted {
        match result {
            Err(RouteCacheKeyDerivationError::CoordinateOutOfRange { key, value }) => {
                assert_eq!(key, "lat");
                assert_eq!(serde_json::Value::Number(value), coordinate);
            }
            other => panic!("expected CoordinateOutOfRange, got {other:?}"),
        }
    }
}

#[test]
fn coordinate_out_of_range_error_matches_snapshot() {
    let error = RouteCacheKey::for_route_request(&json!({"lat": 180.000001}))
        .expect_err("coordinate just outside the bound is rejected");

    assert_snapshot!(
        error.to_string(),
        @"coordinate field 'lat' must be between -180 and 180; got 180.000001"
    );
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
