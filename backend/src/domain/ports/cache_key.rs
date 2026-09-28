//! Domain cache key type and route-request derivation shared by route cache
//! adapters.

use serde_json::{Number, Value};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::idempotency::{PayloadHash, PayloadHashError};

const ROUTE_CACHE_NAMESPACE: &str = "route:v1";
const COORDINATE_PRECISION_FACTOR: f64 = 100_000.0;
const SORTED_ARRAY_KEYS: &[&str] = &["themes", "themeIds", "interestThemeIds"];
const ROUNDED_COORDINATE_KEYS: &[&str] = &["lat", "lng", "lon", "latitude", "longitude"];

/// Cache key used to store and retrieve canonicalized route plans.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RouteCacheKey(String);

impl RouteCacheKey {
    /// Construct a cache key after validating that it is non-empty and trimmed.
    pub fn new(value: impl Into<String>) -> Result<Self, RouteCacheKeyValidationError> {
        let raw = value.into();
        if raw.trim().is_empty() {
            return Err(RouteCacheKeyValidationError::Empty);
        }
        if raw.trim() != raw {
            return Err(RouteCacheKeyValidationError::ContainsWhitespace);
        }
        Ok(Self(raw))
    }

    /// Derive a canonical route cache key from a route request payload.
    ///
    /// The derivation normalizes semantically equivalent route requests so the
    /// cache can be shared across reordered themes, reordered object keys, and
    /// coordinate noise that disappears after rounding to five decimal places.
    ///
    /// # Example
    ///
    /// ```
    /// # use backend::domain::ports::RouteCacheKey;
    /// # use serde_json::json;
    /// let first = json!({
    ///     "origin": {"lat": 51.5000001, "lng": -0.1000001},
    ///     "destination": {"lat": 48.85661, "lng": 2.35222},
    ///     "preferences": {"interestThemeIds": ["b", "a"]},
    /// });
    /// let second = json!({
    ///     "destination": {"lng": 2.35222, "lat": 48.85661},
    ///     "preferences": {"interestThemeIds": ["a", "b"]},
    ///     "origin": {"lng": -0.1, "lat": 51.5},
    /// });
    ///
    /// let first_key = RouteCacheKey::for_route_request(&first).expect("key");
    /// let second_key = RouteCacheKey::for_route_request(&second).expect("key");
    ///
    /// assert_eq!(first_key, second_key);
    /// assert!(first_key.as_str().starts_with("route:v1:"));
    /// ```
    pub fn for_route_request(payload: &Value) -> Result<Self, RouteCacheKeyDerivationError> {
        let hash = hash_route_request_value(payload)?.to_hex();

        // Enforce the namespace/digest contract before the key is accepted.
        if !is_lowercase_hex_digest(&hash) {
            return Err(RouteCacheKeyDerivationError::Validation(
                RouteCacheKeyValidationError::MalformedDigest,
            ));
        }

        Self::new(format!("{ROUTE_CACHE_NAMESPACE}:{hash}"))
            .map_err(RouteCacheKeyDerivationError::Validation)
    }

    /// Borrow the underlying key as a string slice.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl std::fmt::Display for RouteCacheKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl AsRef<str> for RouteCacheKey {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// Validation errors returned when constructing [`RouteCacheKey`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RouteCacheKeyValidationError {
    /// Key is empty after trimming whitespace.
    #[error("route cache key must not be empty")]
    Empty,
    /// Key contains leading or trailing whitespace.
    #[error("route cache key must not contain surrounding whitespace")]
    ContainsWhitespace,
    /// The hash digest embedded in the key is not a 64-character lowercase hex string.
    #[error("route cache key digest must be a 64-character lowercase hex string")]
    MalformedDigest,
}

/// Errors returned while deriving canonical route cache keys.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RouteCacheKeyDerivationError {
    /// The canonical payload could not be hashed.
    #[error(transparent)]
    Hash(#[from] PayloadHashError),
    /// The generated cache key failed validation.
    #[error(transparent)]
    Validation(RouteCacheKeyValidationError),
}

fn hash_route_request_value(value: &Value) -> Result<PayloadHash, PayloadHashError> {
    let normalized = normalize_route_request_value(value, None);
    let json_bytes =
        serde_json::to_vec(&normalized).map_err(|err| PayloadHashError::Serialization {
            message: err.to_string(),
        })?;
    let hash = Sha256::digest(&json_bytes);
    let hash_bytes: [u8; 32] = hash.into();
    Ok(PayloadHash::from_bytes(hash_bytes))
}

fn normalize_route_request_value(value: &Value, current_key: Option<&str>) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<_> = map.iter().collect();
            entries.sort_by_key(|(key, _)| key.as_str());

            let normalized = entries
                .into_iter()
                .map(|(key, child)| {
                    (
                        key.clone(),
                        normalize_route_request_value(child, Some(key.as_str())),
                    )
                })
                .collect();

            Value::Object(normalized)
        }
        Value::Array(items) => {
            let mut normalized: Vec<Value> = items
                .iter()
                .map(|item| normalize_route_request_value(item, None))
                .collect();

            if should_sort_array(current_key) && normalized.iter().all(Value::is_string) {
                normalized.sort_by(|a, b| a.as_str().cmp(&b.as_str()));
            }

            Value::Array(normalized)
        }
        Value::Number(number) if should_round_coordinate(current_key) => {
            Value::Number(round_coordinate(number))
        }
        other => other.clone(),
    }
}

fn should_sort_array(current_key: Option<&str>) -> bool {
    current_key.is_some_and(|key| SORTED_ARRAY_KEYS.contains(&key))
}

fn should_round_coordinate(current_key: Option<&str>) -> bool {
    current_key.is_some_and(|key| ROUNDED_COORDINATE_KEYS.contains(&key))
}

fn round_coordinate(number: &Number) -> Number {
    let Some(value) = number.as_f64() else {
        return number.clone();
    };

    let canonical = round_coordinate_value(value);

    Number::from_f64(canonical).unwrap_or_else(|| number.clone())
}

/// Round to five decimal places and canonicalize signed zero.
///
/// # Examples
///
/// ```
/// # use backend::domain::ports::RouteCacheKey;
/// # use serde_json::json;
/// let rounded = RouteCacheKey::for_route_request(&json!({"lat": 51.500001}))
///     .expect("valid coordinate");
/// let canonical = RouteCacheKey::for_route_request(&json!({"lat": 51.5}))
///     .expect("valid coordinate");
/// assert_eq!(rounded, canonical);
/// let negative_zero = RouteCacheKey::for_route_request(&json!({"lat": -0.0}))
///     .expect("valid coordinate");
/// let positive_zero = RouteCacheKey::for_route_request(&json!({"lat": 0.0}))
///     .expect("valid coordinate");
/// assert_eq!(negative_zero, positive_zero);
/// ```
fn round_coordinate_value(value: f64) -> f64 {
    let rounded = (value * COORDINATE_PRECISION_FACTOR).round() / COORDINATE_PRECISION_FACTOR;
    if rounded == 0.0 { 0.0 } else { rounded }
}

/// Returns `true` when `hash` is a 64-character lowercase hexadecimal string.
fn is_lowercase_hex_digest(hash: &str) -> bool {
    hash.len() == 64
        && hash
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests;
