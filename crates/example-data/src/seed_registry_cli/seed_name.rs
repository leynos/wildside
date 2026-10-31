//! Deterministic seed names using the EFF long word list.

use diceware_wordlists::Wordlist;

use crate::SeedNameError;

use super::CliError;

/// Maps a seed-name domain failure onto the CLI error surface.
impl From<SeedNameError> for CliError {
    fn from(error: SeedNameError) -> Self {
        Self::WordListError {
            message: error.to_string(),
        }
    }
}

/// Returns the EFF long word list in the provider's published order.
///
/// The order is part of the name-compatibility contract: reordering or
/// swapping the provider renames every existing seed, so callers must treat
/// this list as pinned rather than as a convenient word source.
///
/// # Errors
///
/// Returns [`CliError::WordListError`] if the provider yields an empty list,
/// which would make the radix zero.
///
/// # Example
///
/// ```text
/// eff_long_dictionary().map(|words| words.len()) == Ok(7776)
/// ```
pub(super) fn eff_long_dictionary() -> Result<&'static [&'static str], CliError> {
    let words = Wordlist::EffLong.get_list();
    if words.is_empty() {
        return Err(CliError::WordListError {
            message: "EFF word list is empty".to_owned(),
        });
    }
    Ok(words)
}
