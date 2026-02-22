//! Minimal compatibility shim for `shellexpand`.
//!
//! This workspace only requires `shellexpand::tilde` through `base-d`.
//! The full upstream API is intentionally not reimplemented here.

use std::borrow::Cow;

/// Expands a leading `~` to the current user's home directory when available.
///
/// Inputs that do not begin with `~`, or inputs with unsupported forms such as
/// `~other-user`, are returned unchanged.
pub fn tilde<SI: ?Sized + AsRef<str>>(input: &SI) -> Cow<'_, str> {
    let value = input.as_ref();

    if value == "~" {
        return home_dir()
            .map(Cow::Owned)
            .unwrap_or_else(|| Cow::Borrowed(value));
    }

    if let Some(remainder) = value.strip_prefix("~/") {
        return home_dir()
            .map(|home| Cow::Owned(format!("{home}/{remainder}")))
            .unwrap_or_else(|| Cow::Borrowed(value));
    }

    Cow::Borrowed(value)
}

fn home_dir() -> Option<String> {
    std::env::var_os("HOME").map(|path| path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::tilde;

    #[test]
    fn tilde_returns_input_when_not_prefixed() {
        assert_eq!(tilde("/tmp/config").as_ref(), "/tmp/config");
    }

    #[test]
    fn tilde_returns_input_for_unsupported_user_form() {
        assert_eq!(tilde("~other-user/config").as_ref(), "~other-user/config");
    }
}
