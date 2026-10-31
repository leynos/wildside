//! Supplies the EFF long word list to the seed-registry CLI and maps seed-name
//! failures onto its error surface.
//!
//! The radix conversion that turns a seed into a name lives in the domain
//! module `crate::seed_name`, which knows nothing about word-list providers or
//! the CLI. This adapter owns the two things that depend on the provider:
//! [`eff_long_dictionary`] returns the pinned EFF long list in its published
//! order, and the `From<SeedNameError>` implementation converts the domain
//! error into [`CliError::WordListError`]. Keeping the list pinned here is what
//! preserves name compatibility: reordering it, or swapping the provider,
//! renames every existing seed.

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

#[cfg(test)]
mod tests {
    //! Unit tests for the word-list adapter and the error mapping.

    use rstest::rstest;

    use super::{CliError, SeedNameError, eff_long_dictionary};

    #[rstest]
    #[case::empty(SeedNameError::EmptyDictionary, "seed-name dictionary is empty")]
    #[case::too_short(
        SeedNameError::DictionaryTooShort { len: 1 },
        "seed-name dictionary must hold at least two words (got 1)"
    )]
    #[case::out_of_range(
        SeedNameError::IndexOutOfRange { index: 9, len: 3 },
        "seed-name word index 9 is outside a 3-word dictionary"
    )]
    fn domain_errors_surface_as_word_list_errors(
        #[case] error: SeedNameError,
        #[case] expected: &str,
    ) {
        assert_eq!(
            CliError::from(error),
            CliError::WordListError {
                message: expected.to_owned(),
            }
        );
    }

    #[rstest]
    fn the_eff_long_dictionary_holds_the_published_7776_words() {
        let words = eff_long_dictionary().expect("the provider supplies a list");
        assert_eq!(words.len(), 7776);
    }
}
