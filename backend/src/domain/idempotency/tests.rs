//! Unit tests for idempotency primitives.

use super::*;
use rstest::rstest;
use serde_json::json;
use uuid::Uuid;

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
    let hash = PayloadHash::from_bytes(&[0u8; 32]);
    assert_eq!(hash.to_hex().len(), 64);
}

#[test]
fn payload_hash_display_matches_hex() {
    let hash = PayloadHash::from_bytes(&[0xab; 32]);
    assert_eq!(format!("{hash}"), hash.to_hex());
}

// Canonicalization tests

#[test]
fn canonicalize_and_hash_is_deterministic() {
    let value = json!({"foo": "bar", "baz": 123});
    let hash1 = canonicalize_and_hash(&value);
    let hash2 = canonicalize_and_hash(&value);
    assert_eq!(hash1, hash2);
}

#[test]
fn canonicalize_and_hash_ignores_key_order() {
    let a = json!({"z": 1, "a": 2, "m": 3});
    let b = json!({"a": 2, "m": 3, "z": 1});
    assert_eq!(canonicalize_and_hash(&a), canonicalize_and_hash(&b));
}

#[test]
fn canonicalize_and_hash_handles_nested_objects() {
    let a = json!({"outer": {"z": 1, "a": 2}});
    let b = json!({"outer": {"a": 2, "z": 1}});
    assert_eq!(canonicalize_and_hash(&a), canonicalize_and_hash(&b));
}

#[test]
fn canonicalize_and_hash_preserves_array_order() {
    let a = json!({"arr": [1, 2, 3]});
    let b = json!({"arr": [3, 2, 1]});
    assert_ne!(canonicalize_and_hash(&a), canonicalize_and_hash(&b));
}

#[test]
fn canonicalize_and_hash_differs_for_different_values() {
    let a = json!({"key": "value1"});
    let b = json!({"key": "value2"});
    assert_ne!(canonicalize_and_hash(&a), canonicalize_and_hash(&b));
}

#[test]
fn canonicalize_and_hash_handles_primitives() {
    assert_ne!(
        canonicalize_and_hash(&json!(null)),
        canonicalize_and_hash(&json!(false))
    );
    assert_ne!(
        canonicalize_and_hash(&json!(1)),
        canonicalize_and_hash(&json!(2))
    );
    assert_ne!(
        canonicalize_and_hash(&json!("a")),
        canonicalize_and_hash(&json!("b"))
    );
}
