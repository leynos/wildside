//! Idempotency primitives for safe request retries.
//!
//! This module provides types for implementing idempotent request handling:
//!
//! - [`IdempotencyKey`]: Validated UUID identifier sent by clients via the
//!   `Idempotency-Key` HTTP header.
//! - [`PayloadHash`]: SHA-256 hash of a canonicalized request payload, used to
//!   detect conflicting requests for the same key.
//! - [`IdempotencyRecord`]: Stored record linking a key to its payload hash and
//!   original response.
//! - [`IdempotencyLookupResult`]: Outcome of looking up an idempotency key in
//!   the store.
//!
//! # Payload Canonicalization
//!
//! To ensure semantically equivalent payloads produce identical hashes
//! regardless of whitespace or key ordering, payloads are canonicalized before
//! hashing:
//!
//! 1. JSON objects have their keys sorted recursively.
//! 2. The result is serialized to compact JSON (no whitespace).
//! 3. The SHA-256 hash is computed on the resulting bytes.

use std::fmt;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::UserId;

// ---------------------------------------------------------------------------
// IdempotencyKey
// ---------------------------------------------------------------------------

/// Validation errors for [`IdempotencyKey`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IdempotencyKeyValidationError {
    /// The key string was empty.
    EmptyKey,
    /// The key string was not a valid UUID.
    InvalidKey,
}

impl fmt::Display for IdempotencyKeyValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyKey => write!(f, "idempotency key must not be empty"),
            Self::InvalidKey => write!(f, "idempotency key must be a valid UUID"),
        }
    }
}

impl std::error::Error for IdempotencyKeyValidationError {}

/// Client-provided idempotency key (UUID v4).
///
/// Clients send this via the `Idempotency-Key` HTTP header to enable safe
/// request retries. The server uses the key to detect duplicate requests and
/// replay previously computed responses.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct IdempotencyKey(Uuid, String);

impl IdempotencyKey {
    /// Validate and construct an [`IdempotencyKey`] from a string.
    ///
    /// # Errors
    ///
    /// Returns [`IdempotencyKeyValidationError::EmptyKey`] if the input is
    /// empty, or [`IdempotencyKeyValidationError::InvalidKey`] if the input is
    /// not a valid UUID.
    ///
    /// # Example
    ///
    /// ```
    /// # use backend::domain::idempotency::IdempotencyKey;
    /// let key = IdempotencyKey::new("550e8400-e29b-41d4-a716-446655440000")
    ///     .expect("valid UUID");
    /// assert_eq!(key.as_ref(), "550e8400-e29b-41d4-a716-446655440000");
    /// ```
    pub fn new(key: impl AsRef<str>) -> Result<Self, IdempotencyKeyValidationError> {
        Self::from_owned(key.as_ref().to_owned())
    }

    /// Construct an [`IdempotencyKey`] directly from a UUID.
    ///
    /// Useful when the UUID is already validated (e.g., loaded from database).
    pub fn from_uuid(uuid: Uuid) -> Self {
        let raw = uuid.to_string();
        Self(uuid, raw)
    }

    /// Generate a new random [`IdempotencyKey`].
    ///
    /// Primarily useful for testing.
    pub fn random() -> Self {
        let uuid = Uuid::new_v4();
        Self(uuid, uuid.to_string())
    }

    fn from_owned(key: String) -> Result<Self, IdempotencyKeyValidationError> {
        if key.is_empty() {
            return Err(IdempotencyKeyValidationError::EmptyKey);
        }
        if key.trim() != key {
            return Err(IdempotencyKeyValidationError::InvalidKey);
        }
        let parsed =
            Uuid::parse_str(&key).map_err(|_| IdempotencyKeyValidationError::InvalidKey)?;
        Ok(Self(parsed, key))
    }

    /// Access the underlying UUID.
    pub fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl AsRef<str> for IdempotencyKey {
    fn as_ref(&self) -> &str {
        self.1.as_str()
    }
}

impl fmt::Display for IdempotencyKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_ref())
    }
}

impl From<IdempotencyKey> for String {
    fn from(value: IdempotencyKey) -> Self {
        value.1
    }
}

impl TryFrom<String> for IdempotencyKey {
    type Error = IdempotencyKeyValidationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_owned(value)
    }
}

// ---------------------------------------------------------------------------
// PayloadHash
// ---------------------------------------------------------------------------

/// Validation errors for [`PayloadHash`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PayloadHashError {
    /// The byte slice had an incorrect length.
    InvalidLength {
        /// Expected number of bytes.
        expected: usize,
        /// Actual number of bytes.
        actual: usize,
    },
}

impl fmt::Display for PayloadHashError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLength { expected, actual } => {
                write!(f, "payload hash must be {expected} bytes, got {actual}")
            }
        }
    }
}

impl std::error::Error for PayloadHashError {}

/// SHA-256 hash of a canonicalized request payload.
///
/// Used to detect whether two requests with the same idempotency key have
/// identical or conflicting payloads.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PayloadHash([u8; 32]);

impl PayloadHash {
    /// Construct a [`PayloadHash`] from raw bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the slice is not exactly 32 bytes.
    pub fn try_from_bytes(bytes: &[u8]) -> Result<Self, PayloadHashError> {
        let arr: [u8; 32] = bytes
            .try_into()
            .map_err(|_| PayloadHashError::InvalidLength {
                expected: 32,
                actual: bytes.len(),
            })?;
        Ok(Self(arr))
    }

    /// Construct a [`PayloadHash`] from raw bytes.
    ///
    /// # Panics
    ///
    /// Panics if the slice is not exactly 32 bytes. Prefer [`try_from_bytes`]
    /// when handling untrusted input.
    ///
    /// [`try_from_bytes`]: Self::try_from_bytes
    #[expect(
        clippy::expect_used,
        reason = "panic is acceptable for known-valid internal input; use try_from_bytes for external data"
    )]
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self::try_from_bytes(bytes).expect("payload hash must be exactly 32 bytes")
    }

    /// Access the raw hash bytes.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Encode the hash as a lowercase hexadecimal string.
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }
}

impl fmt::Display for PayloadHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_hex())
    }
}

// ---------------------------------------------------------------------------
// Payload canonicalization and hashing
// ---------------------------------------------------------------------------

/// Canonicalize a JSON value and compute its SHA-256 hash.
///
/// Canonicalization ensures semantically equivalent payloads produce identical
/// hashes regardless of whitespace or key ordering:
///
/// 1. Object keys are sorted recursively (lexicographic).
/// 2. Arrays preserve element order.
/// 3. The result is serialized to compact JSON (no whitespace).
/// 4. SHA-256 is computed on the resulting UTF-8 bytes.
///
/// # Example
///
/// ```
/// # use backend::domain::idempotency::canonicalize_and_hash;
/// # use serde_json::json;
/// let a = json!({"b": 2, "a": 1});
/// let b = json!({"a": 1, "b": 2});
/// assert_eq!(canonicalize_and_hash(&a), canonicalize_and_hash(&b));
/// ```
pub fn canonicalize_and_hash(value: &serde_json::Value) -> PayloadHash {
    let canonical = canonicalize(value);
    #[expect(
        clippy::unwrap_used,
        reason = "serde_json::Value serialization to JSON bytes is infallible"
    )]
    let json_bytes = serde_json::to_vec(&canonical).unwrap();
    let hash = Sha256::digest(&json_bytes);
    PayloadHash::from_bytes(&hash)
}

/// Recursively sort object keys for canonical JSON representation.
fn canonicalize(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut sorted: Vec<_> = map.iter().collect();
            sorted.sort_by_key(|(k, _)| *k);
            let canonical_map: serde_json::Map<String, serde_json::Value> = sorted
                .into_iter()
                .map(|(k, v)| (k.clone(), canonicalize(v)))
                .collect();
            serde_json::Value::Object(canonical_map)
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(canonicalize).collect())
        }
        other => other.clone(),
    }
}

// ---------------------------------------------------------------------------
// IdempotencyRecord
// ---------------------------------------------------------------------------

/// Stored idempotency record linking a key to its payload and response.
#[derive(Debug, Clone)]
pub struct IdempotencyRecord {
    /// The idempotency key provided by the client.
    pub key: IdempotencyKey,
    /// SHA-256 hash of the canonicalized request payload.
    pub payload_hash: PayloadHash,
    /// Snapshot of the original response to replay.
    pub response_snapshot: serde_json::Value,
    /// User who made the original request.
    pub user_id: UserId,
    /// When the record was created.
    pub created_at: DateTime<Utc>,
}

impl IdempotencyRecord {
    /// Construct a new record for storage.
    #[expect(
        clippy::too_many_arguments,
        reason = "flat constructor for a domain record with several required fields"
    )]
    pub fn new(
        key: IdempotencyKey,
        payload_hash: PayloadHash,
        response_snapshot: serde_json::Value,
        user_id: UserId,
        created_at: DateTime<Utc>,
    ) -> Self {
        Self {
            key,
            payload_hash,
            response_snapshot,
            user_id,
            created_at,
        }
    }
}

// ---------------------------------------------------------------------------
// IdempotencyLookupResult
// ---------------------------------------------------------------------------

/// Result of looking up an idempotency key in the store.
#[derive(Debug, Clone)]
pub enum IdempotencyLookupResult {
    /// No record exists for this key.
    NotFound,
    /// A record exists and the payload hash matches.
    MatchingPayload(IdempotencyRecord),
    /// A record exists but the payload hash differs (conflict).
    ConflictingPayload(IdempotencyRecord),
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use serde_json::json;

    // IdempotencyKey tests

    #[test]
    fn idempotency_key_accepts_valid_uuid() {
        let key = IdempotencyKey::new("550e8400-e29b-41d4-a716-446655440000");
        assert!(key.is_ok());
        assert_eq!(
            key.unwrap().as_ref(),
            "550e8400-e29b-41d4-a716-446655440000"
        );
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
        let original = IdempotencyKey::new("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let json = serde_json::to_string(&original).unwrap();
        let parsed: IdempotencyKey = serde_json::from_str(&json).unwrap();
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
}
