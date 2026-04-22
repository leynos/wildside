//! Query parameter parsing and normalization for paginated endpoints.
//!
//! This module provides the [`PageParams`] type for parsing and normalizing
//! `cursor` and `limit` query parameters, the [`PageParamsError`] type for
//! validation failures, and the shared constants [`DEFAULT_LIMIT`] and
//! [`MAX_LIMIT`].
//!
//! The [`PageParams`] type implements `Deserialize` with automatic
//! normalization: missing limits default to [`DEFAULT_LIMIT`], oversized limits
//! are capped at [`MAX_LIMIT`], and zero limits are rejected. This behaviour
//! integrates with HTTP framework query extractors (such as Actix Web's
//! `Query<PageParams>`) to ensure consistent limit handling across all
//! endpoints.

use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

/// Shared default page size for endpoints using the pagination foundation.
pub const DEFAULT_LIMIT: usize = 20;
/// Shared maximum page size for endpoints using the pagination foundation.
pub const MAX_LIMIT: usize = 100;

/// Normalized pagination parameters.
///
/// `PageParams` is designed for direct use with query extractors. It applies
/// the shared default limit of 20, caps larger limits at 100, and rejects
/// zero-sized pages.
///
/// # Example
///
/// ```
/// use pagination::PageParams;
///
/// let params = PageParams::new(Some("opaque-token".to_owned()), Some(150))
///     .expect("valid params");
///
/// assert_eq!(params.limit(), 100);
/// assert_eq!(params.cursor(), Some("opaque-token"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PageParams {
    cursor: Option<String>,
    limit: usize,
}

/// Errors raised while normalizing page parameters.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PageParamsError {
    /// The requested limit was zero and cannot represent a page.
    #[error("page limit must be greater than zero")]
    InvalidLimit,
}

#[derive(Debug, Deserialize)]
struct RawPageParams {
    cursor: Option<String>,
    limit: Option<usize>,
}

impl PageParams {
    /// Construct normalized pagination parameters.
    ///
    /// # Errors
    ///
    /// Returns [`PageParamsError::InvalidLimit`] when `limit` is explicitly
    /// set to zero.
    pub fn new(cursor: Option<String>, limit: Option<usize>) -> Result<Self, PageParamsError> {
        let normalized_limit = normalize_limit(limit)?;
        Ok(Self {
            cursor,
            limit: normalized_limit,
        })
    }

    /// Borrow the opaque cursor token, if present.
    #[must_use]
    pub fn cursor(&self) -> Option<&str> {
        self.cursor.as_deref()
    }

    /// Return the normalized page size.
    #[must_use]
    pub const fn limit(&self) -> usize {
        self.limit
    }
}

impl<'de> Deserialize<'de> for PageParams {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = RawPageParams::deserialize(deserializer)?;
        Self::new(raw.cursor, raw.limit).map_err(serde::de::Error::custom)
    }
}

fn normalize_limit(limit: Option<usize>) -> Result<usize, PageParamsError> {
    match limit {
        None => Ok(DEFAULT_LIMIT),
        Some(0) => Err(PageParamsError::InvalidLimit),
        Some(value) => Ok(value.min(MAX_LIMIT)),
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for page parameter normalization.

    use serde_json::json;

    use super::{DEFAULT_LIMIT, MAX_LIMIT, PageParams, PageParamsError};

    #[test]
    fn page_params_default_limit_to_shared_default() {
        let params = PageParams::new(None, None).expect("default params should be valid");

        assert_eq!(params.limit(), DEFAULT_LIMIT);
        assert_eq!(params.cursor(), None);
    }

    #[test]
    fn page_params_cap_limit_to_shared_maximum() {
        let params = PageParams::new(Some("opaque".to_owned()), Some(MAX_LIMIT + 50))
            .expect("oversized limit should clamp");

        assert_eq!(params.limit(), MAX_LIMIT);
        assert_eq!(params.cursor(), Some("opaque"));
    }

    #[test]
    fn page_params_accepts_limit_one_below_maximum() {
        let params = PageParams::new(None, Some(MAX_LIMIT - 1))
            .expect("limit one below maximum should be valid");
        assert_eq!(
            params.limit(),
            MAX_LIMIT - 1,
            "limit below MAX_LIMIT should pass through unchanged"
        );
    }

    #[test]
    fn page_params_clamps_limit_one_above_maximum() {
        let params = PageParams::new(None, Some(MAX_LIMIT + 1))
            .expect("limit one above maximum should clamp");
        assert_eq!(
            params.limit(),
            MAX_LIMIT,
            "limit one above MAX_LIMIT should be clamped to MAX_LIMIT"
        );
    }

    #[test]
    fn page_params_reject_zero_limit() {
        let result = PageParams::new(None, Some(0));

        assert_eq!(result, Err(PageParamsError::InvalidLimit));
    }

    #[test]
    fn page_params_deserialization_normalizes_limit() {
        let params: PageParams = serde_json::from_value(json!({
            "cursor": "opaque",
            "limit": 999
        }))
        .expect("deserialization should succeed");

        assert_eq!(params.limit(), MAX_LIMIT);
        assert_eq!(params.cursor(), Some("opaque"));
    }
}
