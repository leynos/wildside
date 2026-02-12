//! Shared slug validation predicates for domain entities.
//!
//! Slugs are trimmed, non-empty identifiers composed of lowercase ASCII
//! letters, digits, and hyphens.

/// Return `true` when `value` is a valid domain slug.
///
/// # Examples
///
/// ```rust,ignore
/// use backend::domain::slug::is_valid_slug;
///
/// assert!(is_valid_slug("coastal-loop"));
/// assert!(!is_valid_slug("Coastal Loop"));
/// ```
pub(crate) fn is_valid_slug(value: &str) -> bool {
    is_trimmed_non_empty(value) && has_allowed_slug_chars(value)
}

fn is_trimmed_non_empty(value: &str) -> bool {
    !value.is_empty() && value.trim() == value
}

fn has_allowed_slug_chars(value: &str) -> bool {
    value
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
}
