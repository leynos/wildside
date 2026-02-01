//! Payload hashing and canonicalization helpers.

use std::fmt;

use sha2::{Digest, Sha256};

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

    /// Construct a [`PayloadHash`] from a 32-byte array.
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
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
    let hash_bytes: [u8; 32] = hash.into();
    PayloadHash::from_bytes(hash_bytes)
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
