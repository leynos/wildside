//! Utilities for validating user display names.
use once_cell::sync::Lazy;
use regex::Regex;

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
pub fn is_valid_display_name(name: &str) -> bool {
    static RE: Lazy<Regex> = Lazy::new(|| {
        let pattern = format!("^[A-Za-z0-9_ ]{{{DISPLAY_NAME_MIN},{DISPLAY_NAME_MAX}}}$");
        Regex::new(&pattern).expect("valid regex")
    });
    RE.is_match(name)
}
