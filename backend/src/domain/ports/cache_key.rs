//! Domain cache key type and route-request derivation shared by route cache
//! adapters.

use serde_json::{Number, Value};
use thiserror::Error;

use crate::domain::canonicalize_and_hash;
use crate::domain::idempotency::PayloadHashError;

const ROUTE_CACHE_NAMESPACE: &str = "route:v1";
const COORDINATE_PRECISION_FACTOR: f64 = 100_000.0;
const SORTED_ARRAY_KEYS: &[&str] = &["themes", "themeIds", "interestThemeIds"];
const ROUNDED_COORDINATE_KEYS: &[&str] = &["lat", "lng", "lon", "latitude", "longitude"];

/// Cache key used to store and retrieve canonicalised route plans.
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
        let normalized = normalize_route_request_value(payload, None);
        let hash = canonicalize_and_hash(&normalized)?;

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
                normalized.sort_by_key(|value| value.to_string());
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

    let rounded = (value * COORDINATE_PRECISION_FACTOR).round() / COORDINATE_PRECISION_FACTOR;
    let canonical = if rounded == 0.0 { 0.0 } else { rounded };

    Number::from_f64(canonical).unwrap_or_else(|| number.clone())
}

#[cfg(test)]
mod tests {
    //! Validates cache key parsing, canonicalization, and whitespace
    //! constraints.
    use serde_json::json;

    use super::{
        ROUNDED_COORDINATE_KEYS, RouteCacheKey, RouteCacheKeyValidationError, SORTED_ARRAY_KEYS,
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
}
