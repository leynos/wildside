//! Utilities for validating user display names.
use regex::Regex;
use std::sync::OnceLock;

/// Minimum allowed length for a display name.
pub const DISPLAY_NAME_MIN: usize = 3;
/// Maximum allowed length for a display name.
pub const DISPLAY_NAME_MAX: usize = 32;

/// Return `Ok(true)` if the display name matches policy.
///
/// Only alphanumeric characters, underscores and spaces are allowed.
///
/// ```
/// use wildside::ws::display_name::is_valid_display_name;
/// assert!(matches!(is_valid_display_name("Alice"), Ok(true)));
/// assert!(matches!(is_valid_display_name("bad$char"), Ok(false)));
/// ```
static DISPLAY_NAME_RE: OnceLock<Result<Regex, regex::Error>> = OnceLock::new();

fn get_display_name_regex() -> Result<&'static Regex, regex::Error> {
    DISPLAY_NAME_RE
        .get_or_init(|| {
            let pattern = format!("^[A-Za-z0-9_ ]{{{DISPLAY_NAME_MIN},{DISPLAY_NAME_MAX}}}$");
            Regex::new(&pattern)
        })
        .as_ref()
        .map_err(Clone::clone)
}

pub fn is_valid_display_name(name: &str) -> Result<bool, regex::Error> {
    Ok(get_display_name_regex()?.is_match(name))
}
