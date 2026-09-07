//! Unit tests for idempotency primitives.

use std::time::Duration;

use rstest::rstest;
use serde_json::json;
use uuid::Uuid;

use super::*;

// IdempotencyKey tests

#[test]
fn idempotency_key_accepts_valid_uuid() {
    let key = IdempotencyKey::new("550e8400-e29b-41d4-a716-446655440000")
        .expect("valid UUID should parse");
    assert_eq!(key.as_ref(), "550e8400-e29b-41d4-a716-446655440000");
}

#[test]
fn idempotency_key_rejects_empty_string() {
    let key = IdempotencyKey::new("");
    assert!(matches!(key, Err(IdempotencyKeyValidationError::EmptyKey)));
}

#[rstest]
#[case("not-a-uuid")]
#[case("550e8400-e29b-41d4-a716")]
#[case(" 550e8400-e29b-41d4-a716-446655440000")]
#[case("550e8400-e29b-41d4-a716-446655440000 ")]
fn idempotency_key_rejects_invalid_format(#[case] input: &str) {
    let key = IdempotencyKey::new(input);
    assert!(matches!(
        key,
        Err(IdempotencyKeyValidationError::InvalidKey)
    ));
}

#[test]
fn idempotency_key_from_uuid_roundtrip() {
    let uuid = Uuid::new_v4();
    let key = IdempotencyKey::from_uuid(uuid);
    assert_eq!(key.as_uuid(), &uuid);
}

#[test]
fn idempotency_key_serde_roundtrip() {
    let original = IdempotencyKey::new("550e8400-e29b-41d4-a716-446655440000")
        .expect("valid UUID should parse");
    let json = serde_json::to_string(&original).expect("serialization should succeed");
    let parsed: IdempotencyKey =
        serde_json::from_str(&json).expect("deserialization should succeed");
    assert_eq!(original, parsed);
}

// PayloadHash tests

#[test]
fn payload_hash_to_hex_produces_64_chars() {
    let hash = PayloadHash::from_bytes([0u8; 32]);
    assert_eq!(hash.to_hex().len(), 64);
}

#[test]
fn payload_hash_display_matches_hex() {
    let hash = PayloadHash::from_bytes([0xab; 32]);
    assert_eq!(format!("{hash}"), hash.to_hex());
}

#[test]
fn payload_hash_rejects_invalid_length() {
    let err = PayloadHash::try_from_bytes(&[0u8; 31]).expect_err("expected length error");
    assert_eq!(
        err,
        PayloadHashError::InvalidLength {
            expected: 32,
            actual: 31,
        }
    );
}

// Canonicalization tests

#[test]
fn canonicalize_and_hash_is_deterministic() {
    let value = json!({"foo": "bar", "baz": 123});
    let hash1 = canonicalize_and_hash(&value).expect("hash value");
    let hash2 = canonicalize_and_hash(&value).expect("hash value");
    assert_eq!(hash1, hash2);
}

#[test]
fn canonicalize_and_hash_ignores_key_order() {
    let a = json!({"z": 1, "a": 2, "m": 3});
    let b = json!({"a": 2, "m": 3, "z": 1});
    let hash_a = canonicalize_and_hash(&a).expect("hash a");
    let hash_b = canonicalize_and_hash(&b).expect("hash b");
    assert_eq!(hash_a, hash_b);
}

#[test]
fn canonicalize_and_hash_handles_nested_objects() {
    let a = json!({"outer": {"z": 1, "a": 2}});
    let b = json!({"outer": {"a": 2, "z": 1}});
    let hash_a = canonicalize_and_hash(&a).expect("hash a");
    let hash_b = canonicalize_and_hash(&b).expect("hash b");
    assert_eq!(hash_a, hash_b);
}

#[test]
fn canonicalize_and_hash_preserves_array_order() {
    let a = json!({"arr": [1, 2, 3]});
    let b = json!({"arr": [3, 2, 1]});
    let hash_a = canonicalize_and_hash(&a).expect("hash a");
    let hash_b = canonicalize_and_hash(&b).expect("hash b");
    assert_ne!(hash_a, hash_b);
}

#[test]
fn canonicalize_and_hash_differs_for_different_values() {
    let a = json!({"key": "value1"});
    let b = json!({"key": "value2"});
    let hash_a = canonicalize_and_hash(&a).expect("hash a");
    let hash_b = canonicalize_and_hash(&b).expect("hash b");
    assert_ne!(hash_a, hash_b);
}

#[test]
fn canonicalize_and_hash_handles_primitives() {
    let null_hash = canonicalize_and_hash(&json!(null)).expect("hash null");
    let false_hash = canonicalize_and_hash(&json!(false)).expect("hash false");
    assert_ne!(null_hash, false_hash);

    let one_hash = canonicalize_and_hash(&json!(1)).expect("hash one");
    let two_hash = canonicalize_and_hash(&json!(2)).expect("hash two");
    assert_ne!(one_hash, two_hash);

    let a_hash = canonicalize_and_hash(&json!("a")).expect("hash a");
    let b_hash = canonicalize_and_hash(&json!("b")).expect("hash b");
    assert_ne!(a_hash, b_hash);
}

// MutationType tests

#[rstest]
#[case(MutationType::Routes, "routes")]
#[case(MutationType::Notes, "notes")]
#[case(MutationType::Progress, "progress")]
#[case(MutationType::Preferences, "preferences")]
#[case(MutationType::Bundles, "bundles")]
fn mutation_type_as_str(#[case] mutation: MutationType, #[case] expected: &str) {
    assert_eq!(mutation.as_str(), expected);
}

#[rstest]
#[case(MutationType::Routes, "routes")]
#[case(MutationType::Notes, "notes")]
#[case(MutationType::Progress, "progress")]
#[case(MutationType::Preferences, "preferences")]
#[case(MutationType::Bundles, "bundles")]
fn mutation_type_display(#[case] mutation: MutationType, #[case] expected: &str) {
    assert_eq!(format!("{mutation}"), expected);
}

#[rstest]
#[case("routes", MutationType::Routes)]
#[case("notes", MutationType::Notes)]
#[case("progress", MutationType::Progress)]
#[case("preferences", MutationType::Preferences)]
#[case("bundles", MutationType::Bundles)]
fn mutation_type_from_str(#[case] input: &str, #[case] expected: MutationType) {
    use std::str::FromStr;
    assert_eq!(
        MutationType::from_str(input).expect("valid input"),
        expected
    );
}

#[rstest]
#[case("invalid")]
#[case("Routes")]
#[case("ROUTES")]
#[case("")]
fn mutation_type_from_str_rejects_invalid(#[case] input: &str) {
    use std::str::FromStr;
    let result = MutationType::from_str(input);
    assert!(result.is_err(), "expected error for input '{input}'");
    let err = result.unwrap_err();
    assert_eq!(err.input, input);
}

#[test]
fn mutation_type_serde_roundtrip() {
    for mutation in [
        MutationType::Routes,
        MutationType::Notes,
        MutationType::Progress,
        MutationType::Preferences,
        MutationType::Bundles,
    ] {
        let json = serde_json::to_string(&mutation).expect("serialization should succeed");
        let parsed: MutationType =
            serde_json::from_str(&json).expect("deserialization should succeed");
        assert_eq!(mutation, parsed);
    }
}

#[test]
fn mutation_type_serializes_to_snake_case() {
    let json = serde_json::to_string(&MutationType::Routes).expect("serialization should succeed");
    assert_eq!(json, "\"routes\"");
}

/// Validates that MutationType::ALL matches the CHECK constraint in the migration.
///
/// If this test fails, you likely added a new MutationType variant. You must also
/// update the CHECK constraint in:
/// `backend/migrations/2025-12-28-000000_add_mutation_type_to_idempotency_keys/up.sql`
#[test]
fn mutation_type_values_match_migration_constraint() {
    use std::collections::HashSet;

    // These values must match the CHECK constraint in the migration file:
    // backend/migrations/2025-12-28-000000_add_mutation_type_to_idempotency_keys/up.sql
    let migration_values: HashSet<&str> = ["routes", "notes", "progress", "preferences", "bundles"]
        .into_iter()
        .collect();

    let code_values: HashSet<&str> = MutationType::ALL.iter().map(|m| m.as_str()).collect();

    assert_eq!(
        code_values, migration_values,
        "MutationType::ALL does not match migration CHECK constraint. \
         If you added a variant, update the migration CHECK constraint in \
         backend/migrations/2025-12-28-000000_add_mutation_type_to_idempotency_keys/up.sql"
    );
}

// IdempotencyConfig tests
//
// Environment-driven loading now lives in `crate::config::idempotency`; the
// tests below cover only the pure value object.

/// Scenario: the configuration is built from its default and from an explicit
/// duration.
///
/// Invariant: the default TTL is 24 hours and an explicit duration is stored
/// verbatim.
#[test]
fn idempotency_config_default_is_24_hours() {
    let config = IdempotencyConfig::default();
    assert_eq!(config.ttl(), Duration::from_secs(24 * 3600));
}

#[test]
fn idempotency_config_with_ttl_sets_custom_duration() {
    let ttl = Duration::from_secs(12 * 3600);
    let config = IdempotencyConfig::with_ttl(ttl);
    assert_eq!(config.ttl(), ttl);
}

/// Scenario: an hours value is absent, in range, or outside the permitted
/// range.
///
/// Invariant: `None` selects the default and any supplied value is clamped to
/// `[MIN_TTL_HOURS, MAX_TTL_HOURS]`, with no environment access.
#[rstest]
#[case::absent(None, 24 * 3600)]
#[case::in_range(Some(48), 48 * 3600)]
#[case::below_minimum(Some(0), 3600)]
#[case::above_maximum(Some(999_999), 87600 * 3600)]
fn idempotency_config_from_ttl_hours_clamps(
    #[case] hours: Option<u64>,
    #[case] expected_seconds: u64,
) {
    let config = IdempotencyConfig::from_ttl_hours(hours);
    assert_eq!(
        config.ttl(),
        Duration::from_secs(expected_seconds),
        "the TTL must be clamped to the domain's permitted range"
    );
}
