//! Display name validation mirroring backend constraints.
//!
//! This module provides validation rules that match the backend's
//! `DisplayName` type in `backend/src/domain/user.rs`. Keeping these rules
//! in sync ensures generated names are always valid when consumed by the
//! backend.
//!
//! # Validation Rules
//!
//! - Minimum length: 3 characters
//! - Maximum length: 32 characters
//! - Allowed characters: letters (A-Z, a-z), digits (0-9), spaces, underscores
//! - Must not be whitespace-only

/// Minimum allowed length for a display name.
pub const DISPLAY_NAME_MIN: usize = 3;

/// Maximum allowed length for a display name.
pub const DISPLAY_NAME_MAX: usize = 32;

/// Validates a display name against backend constraints.
///
/// Returns `true` if the name satisfies all validation rules:
/// - Length between [`DISPLAY_NAME_MIN`] and [`DISPLAY_NAME_MAX`] characters
/// - Contains only alphanumeric characters, spaces, and underscores
/// - Is not whitespace-only
///
/// # Examples
///
/// ```
/// use example_data::is_valid_display_name;
///
/// assert!(is_valid_display_name("Ada Lovelace"));
/// assert!(is_valid_display_name("user_123"));
/// assert!(!is_valid_display_name("ab"));           // Too short
/// assert!(!is_valid_display_name("O'Brien"));      // Invalid character
/// assert!(!is_valid_display_name("   "));          // Whitespace-only
/// ```
#[must_use]
pub fn is_valid_display_name(name: &str) -> bool {
    let length = name.chars().count();
    if !(DISPLAY_NAME_MIN..=DISPLAY_NAME_MAX).contains(&length) {
        return false;
    }
    // Reject whitespace-only names (mirrors backend's trim().is_empty() check)
    if name.trim().is_empty() {
        return false;
    }
    name.chars().all(is_valid_display_name_char)
}

/// Returns `true` if the character is allowed in a display name.
///
/// Allowed characters are:
/// - ASCII alphanumeric characters (A-Z, a-z, 0-9)
/// - Spaces
/// - Underscores
#[must_use]
const fn is_valid_display_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == ' ' || c == '_'
}

/// Sanitizes a raw name by replacing invalid characters with underscores.
///
/// This function transforms a name that may contain invalid characters into
/// one that matches the display name pattern. It does not enforce length
/// constraints.
#[must_use]
pub(crate) fn sanitize_display_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if is_valid_display_name_char(c) {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    //! Covers display name validation and sanitisation behaviour.

    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case("Ada", true)]
    #[case("Ada Lovelace", true)]
    #[case("user_123", true)]
    #[case("A B C", true)]
    #[case("abc", true)]
    #[case("ABC123", true)]
    #[case("Test User Name Here", true)]
    fn valid_display_names(#[case] name: &str, #[case] expected: bool) {
        assert_eq!(is_valid_display_name(name), expected);
    }

    #[rstest]
    #[case("ab", false)] // Too short
    #[case("a", false)] // Too short
    #[case("", false)] // Empty
    #[case("O'Brien", false)] // Apostrophe
    #[case("Marie-Claire", false)] // Hyphen
    #[case("user@email", false)] // At sign
    #[case("hello!", false)] // Exclamation
    #[case("   ", false)] // Whitespace-only (3 spaces)
    #[case("     ", false)] // Whitespace-only (5 spaces)
    fn invalid_display_names(#[case] name: &str, #[case] expected: bool) {
        assert_eq!(is_valid_display_name(name), expected);
    }

    #[test]
    fn rejects_names_exceeding_max_length() {
        let long_name = "A".repeat(DISPLAY_NAME_MAX + 1);
        assert!(!is_valid_display_name(&long_name));
    }

    #[test]
    fn accepts_names_at_exact_min_length() {
        let min_name = "A".repeat(DISPLAY_NAME_MIN);
        assert!(is_valid_display_name(&min_name));
    }

    #[test]
    fn accepts_names_at_exact_max_length() {
        let max_name = "A".repeat(DISPLAY_NAME_MAX);
        assert!(is_valid_display_name(&max_name));
    }

    #[test]
    fn sanitize_replaces_apostrophes() {
        assert_eq!(sanitize_display_name("O'Brien"), "O_Brien");
    }

    #[test]
    fn sanitize_replaces_hyphens() {
        assert_eq!(sanitize_display_name("Marie-Claire"), "Marie_Claire");
    }

    #[test]
    fn sanitize_preserves_valid_characters() {
        assert_eq!(sanitize_display_name("Ada Lovelace"), "Ada Lovelace");
    }

    #[test]
    fn sanitize_handles_multiple_invalid_chars() {
        assert_eq!(sanitize_display_name("a-b'c@d!e"), "a_b_c_d_e");
    }
}
