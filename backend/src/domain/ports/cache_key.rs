//! Domain cache key type shared by route cache adapters.
use thiserror::Error;

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

#[cfg(test)]
mod tests {
    //! Validates cache key parsing and whitespace constraints.
    use super::{RouteCacheKey, RouteCacheKeyValidationError};
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
}
