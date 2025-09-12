//! Utilities for validating user display names.
use regex::Regex;
use std::sync::LazyLock;

/// Minimum allowed length for a display name.
pub const DISPLAY_NAME_MIN: usize = 3;
/// Maximum allowed length for a display name.
pub const DISPLAY_NAME_MAX: usize = 32;

/// Return true if the display name matches policy.
///
/// Only alphanumeric characters, underscores and spaces are allowed.
///
/// ```
/// assert!(wildside::ws::display_name::is_valid_display_name("Alice"));
/// assert!(!wildside::ws::display_name::is_valid_display_name("bad$char"));
/// ```
static DISPLAY_NAME_RE: LazyLock<Regex> = LazyLock::new(|| {
    let pattern = format!("^[A-Za-z0-9_ ]{{{DISPLAY_NAME_MIN},{DISPLAY_NAME_MAX}}}$");
    Regex::new(&pattern).unwrap_or_else(|e| unreachable!("invalid display name regex: {e}"))
});

pub fn is_valid_display_name(name: &str) -> bool {
    DISPLAY_NAME_RE.is_match(name)
}
