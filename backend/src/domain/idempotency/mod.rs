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
//! - [`MutationType`]: Discriminator for different outbox-backed operations.
//! - [`IdempotencyConfig`]: Configuration for idempotency TTL.
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

#[cfg(test)]
mod tests;

use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::UserId;

// ---------------------------------------------------------------------------
// MutationType
// ---------------------------------------------------------------------------

/// The type of mutation protected by idempotency.
///
/// Each variant corresponds to an outbox-backed operation that supports
/// idempotent retries. The discriminator ensures keys are isolated per
/// mutation kind, preventing collisions when different operations use
/// the same UUID.
///
/// # Example
///
/// ```
/// # use backend::domain::idempotency::MutationType;
/// let mutation = MutationType::Routes;
/// assert_eq!(mutation.as_str(), "routes");
/// assert_eq!(mutation.to_string(), "routes");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationType {
    /// Route submission (`POST /api/v1/routes`).
    Routes,
    /// Route note upsert (`POST /api/v1/routes/{route_id}/notes`).
    Notes,
    /// Route progress update (`PUT /api/v1/routes/{route_id}/progress`).
    Progress,
    /// User preferences update (`PUT /api/v1/users/me/preferences`).
    Preferences,
    /// Offline bundle operations (`POST/DELETE /api/v1/offline/bundles`).
    Bundles,
}

impl MutationType {
    /// All mutation type variants.
    ///
    /// Useful for iteration, validation, and documentation.
    pub const ALL: [MutationType; 5] = [
        MutationType::Routes,
        MutationType::Notes,
        MutationType::Progress,
        MutationType::Preferences,
        MutationType::Bundles,
    ];
}

impl MutationType {
    /// Returns the database string representation.
    ///
    /// # Example
    ///
    /// ```
    /// # use backend::domain::idempotency::MutationType;
    /// assert_eq!(MutationType::Routes.as_str(), "routes");
    /// assert_eq!(MutationType::Notes.as_str(), "notes");
    /// assert_eq!(MutationType::Progress.as_str(), "progress");
    /// assert_eq!(MutationType::Preferences.as_str(), "preferences");
    /// assert_eq!(MutationType::Bundles.as_str(), "bundles");
    /// ```
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Routes => "routes",
            Self::Notes => "notes",
            Self::Progress => "progress",
            Self::Preferences => "preferences",
            Self::Bundles => "bundles",
        }
    }
}

impl fmt::Display for MutationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Error returned when parsing an invalid mutation type string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseMutationTypeError {
    /// The invalid input string.
    pub input: String,
}

impl fmt::Display for ParseMutationTypeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let variants: Vec<_> = MutationType::ALL.iter().map(|v| v.as_str()).collect();
        write!(
            f,
            "invalid mutation type '{}': expected one of {}",
            self.input,
            variants.join(", ")
        )
    }
}

impl std::error::Error for ParseMutationTypeError {}

impl FromStr for MutationType {
    type Err = ParseMutationTypeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .iter()
            .find(|v| v.as_str() == s)
            .copied()
            .ok_or_else(|| ParseMutationTypeError {
                input: s.to_owned(),
            })
    }
}

// ---------------------------------------------------------------------------
// IdempotencyConfig
// ---------------------------------------------------------------------------

/// Environment variable name for idempotency TTL configuration.
pub const IDEMPOTENCY_TTL_HOURS_ENV: &str = "IDEMPOTENCY_TTL_HOURS";

/// Environment abstraction for idempotency configuration lookups.
///
/// This trait allows testing with mock environments without unsafe env var
/// mutations.
pub trait IdempotencyEnv {
    /// Fetch a string value by name.
    fn string(&self, name: &str) -> Option<String>;
}

/// Environment access backed by the real process environment.
#[derive(Clone, Copy, Debug, Default)]
pub struct DefaultIdempotencyEnv;

impl DefaultIdempotencyEnv {
    /// Create a new environment reader.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl IdempotencyEnv for DefaultIdempotencyEnv {
    fn string(&self, name: &str) -> Option<String> {
        std::env::var(name).ok()
    }
}

/// Configuration for idempotency behaviour.
///
/// Controls the time-to-live (TTL) for idempotency records. Records older than
/// the TTL are eligible for cleanup.
///
/// # Example
///
/// ```
/// # use backend::domain::idempotency::IdempotencyConfig;
/// # use std::time::Duration;
/// let config = IdempotencyConfig::default();
/// assert_eq!(config.ttl(), Duration::from_secs(24 * 3600));
///
/// let custom = IdempotencyConfig::with_ttl(Duration::from_secs(12 * 3600));
/// assert_eq!(custom.ttl(), Duration::from_secs(12 * 3600));
/// ```
#[derive(Debug, Clone)]
pub struct IdempotencyConfig {
    ttl: Duration,
}

impl IdempotencyConfig {
    /// Default TTL in hours.
    const DEFAULT_TTL_HOURS: u64 = 24;

    /// Minimum allowed TTL in hours.
    ///
    /// Prevents pathologically short TTLs that would cause records to expire
    /// before retries can complete.
    const MIN_TTL_HOURS: u64 = 1;

    /// Maximum allowed TTL in hours (10 years).
    ///
    /// Prevents pathologically long TTLs that could cause database bloat or
    /// overflow issues.
    const MAX_TTL_HOURS: u64 = 24 * 365 * 10;

    /// Load configuration from the real process environment.
    ///
    /// Reads `IDEMPOTENCY_TTL_HOURS` (default: 24). Values are clamped to
    /// the range \[1, 87600\] (1 hour to 10 years) to prevent pathological
    /// configurations.
    ///
    /// # Example
    ///
    /// ```
    /// # use backend::domain::idempotency::IdempotencyConfig;
    /// # use std::time::Duration;
    /// // Without env var set, uses default of 24 hours
    /// let config = IdempotencyConfig::from_env();
    /// assert_eq!(config.ttl(), Duration::from_secs(24 * 3600));
    /// ```
    pub fn from_env() -> Self {
        Self::from_env_with(&DefaultIdempotencyEnv)
    }

    /// Load configuration from a custom environment source.
    ///
    /// Useful for testing without unsafe env var mutations.
    pub fn from_env_with(env: &impl IdempotencyEnv) -> Self {
        let hours = env
            .string(IDEMPOTENCY_TTL_HOURS_ENV)
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(Self::DEFAULT_TTL_HOURS)
            .clamp(Self::MIN_TTL_HOURS, Self::MAX_TTL_HOURS);
        Self {
            ttl: Duration::from_secs(hours.saturating_mul(3600)),
        }
    }

    /// Create with explicit TTL (for testing).
    pub fn with_ttl(ttl: Duration) -> Self {
        Self { ttl }
    }

    /// Returns the configured TTL.
    pub fn ttl(&self) -> Duration {
        self.ttl
    }
}

impl Default for IdempotencyConfig {
    fn default() -> Self {
        Self {
            ttl: Duration::from_secs(Self::DEFAULT_TTL_HOURS * 3600),
        }
    }
}

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
    /// The type of mutation this record protects.
    pub mutation_type: MutationType,
    /// SHA-256 hash of the canonicalized request payload.
    pub payload_hash: PayloadHash,
    /// Snapshot of the original response to replay.
    pub response_snapshot: serde_json::Value,
    /// User who made the original request.
    pub user_id: UserId,
    /// When the record was created.
    pub created_at: DateTime<Utc>,
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
// IdempotencyLookupQuery
// ---------------------------------------------------------------------------

/// Query parameters for looking up an idempotency key.
///
/// Bundles the parameters needed for an idempotency lookup into a single struct,
/// reducing the number of arguments passed to repository methods.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdempotencyLookupQuery {
    /// The idempotency key to look up.
    pub key: IdempotencyKey,
    /// The user who made the request.
    pub user_id: UserId,
    /// The type of mutation being performed.
    pub mutation_type: MutationType,
    /// The hash of the request payload.
    pub payload_hash: PayloadHash,
}

impl IdempotencyLookupQuery {
    /// Create a new lookup query.
    pub fn new(
        key: IdempotencyKey,
        user_id: UserId,
        mutation_type: MutationType,
        payload_hash: PayloadHash,
    ) -> Self {
        Self {
            key,
            user_id,
            mutation_type,
            payload_hash,
        }
    }
}
