//! Deterministic seed names using the EFF long word list.

use diceware_wordlists::Wordlist;
use num_bigint::BigUint;

use super::CliError;

/// Encodes a seed as an EFF-long word name using the supplied dictionary.
///
/// The encoding is the one the retired `base-d` dependency produced, because
/// seed names are a compatibility surface: the same seed must keep the same
/// name forever, or existing registries silently change identity. The seed is
/// therefore rendered as a decimal string, those bytes are read as one
/// big-endian integer, and that integer is converted to base
/// `dictionary.len()` by repeated division. Words are emitted most significant
/// first and joined with `-`.
///
/// Note that this is decimal-byte input, not a byte-oriented base such as
/// base64: a seed of `0` yields the single word at index zero, because the
/// decimal string `"0"` is not a string of zero bytes.
///
/// # Errors
///
/// Returns [`CliError::WordListError`] when the dictionary is empty, which
/// would otherwise divide by zero, or when a computed index falls outside it.
///
/// # Example
///
/// ```text
/// seed_name_from_value(2026, &words) == Ok("abroad-unmanaged-greasily".to_owned())
/// ```
pub(super) fn seed_name_from_value(seed: u64, dictionary: &[&str]) -> Result<String, CliError> {
    let radix = BigUint::from(dictionary.len());
    if radix == BigUint::from(0u8) {
        return Err(CliError::WordListError {
            message: "EFF word list is empty".to_owned(),
        });
    }

    // Keep the original encoding of decimal UTF-8 bytes so existing seeds keep
    // their names when the word-list provider changes.
    let mut number = BigUint::from_bytes_be(seed.to_string().as_bytes());
    let mut words = Vec::new();
    while number != BigUint::from(0u8) {
        let remainder = (&number % &radix).to_u32_digits();
        let index = remainder.first().copied().unwrap_or(0) as usize;
        let word = dictionary
            .get(index)
            .ok_or_else(|| CliError::WordListError {
                message: "word index outside EFF dictionary".to_owned(),
            })?;
        words.push(*word);
        number /= &radix;
    }
    words.reverse();
    Ok(words.join("-"))
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
