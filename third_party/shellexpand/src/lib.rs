//! Minimal compatibility shim for `shellexpand`.
//!
//! This workspace only requires `shellexpand::tilde` through `base-d`.
//! The full upstream API is intentionally not reimplemented here.

use std::borrow::Cow;

/// Expands a leading `~` to the current user's home directory when available.
///
/// Inputs that do not begin with `~`, or inputs with unsupported forms such as
/// `~other-user`, are returned unchanged.
///
/// # Examples
///
/// ```rust
/// use shellexpand::tilde;
///
/// assert_eq!(tilde("/tmp/config").as_ref(), "/tmp/config");
/// ```
pub fn tilde<SI: ?Sized + AsRef<str>>(input: &SI) -> Cow<'_, str> {
    let home = std::env::var("HOME").ok();
    tilde_with_home(input.as_ref(), home.as_deref())
}

fn tilde_with_home<'a>(input: &'a str, home: Option<&str>) -> Cow<'a, str> {
    if input == "~" {
        return home
            .map(|home_dir| Cow::Owned(home_dir.to_owned()))
            .unwrap_or_else(|| Cow::Borrowed(input));
    }

    if let Some(remainder) = input.strip_prefix("~/") {
        return home
            .map(|home_dir| Cow::Owned(format!("{home_dir}/{remainder}")))
            .unwrap_or_else(|| Cow::Borrowed(input));
    }

    Cow::Borrowed(input)
}

#[cfg(test)]
mod tests {
    use super::{tilde, tilde_with_home};

    #[test]
    fn tilde_with_home_expands_root_tilde() {
        assert_eq!(tilde_with_home("~", Some("/home/test")).as_ref(), "/home/test");
    }

    #[test]
    fn tilde_with_home_expands_tilde_prefix() {
        assert_eq!(
            tilde_with_home("~/config", Some("/home/test")).as_ref(),
            "/home/test/config"
        );
    }

    #[test]
    fn tilde_with_home_preserves_root_tilde_without_home() {
        assert_eq!(tilde_with_home("~", None).as_ref(), "~");
    }

    #[test]
    fn tilde_with_home_preserves_tilde_prefix_without_home() {
        assert_eq!(tilde_with_home("~/config", None).as_ref(), "~/config");
    }

    #[test]
    fn tilde_returns_input_when_not_prefixed() {
        assert_eq!(tilde("/tmp/config").as_ref(), "/tmp/config");
    }

    #[test]
    fn tilde_returns_input_for_unsupported_user_form() {
        assert_eq!(tilde("~other-user/config").as_ref(), "~other-user/config");
    }
}
