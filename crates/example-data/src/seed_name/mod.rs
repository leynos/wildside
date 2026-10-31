//! Deterministic seed-name encoding over a supplied word list.
//!
//! Seed names are a compatibility surface: the same seed must keep the same
//! name forever, or existing registries silently change identity. This module
//! holds that policy alone, with no filesystem, CLI, or provider dependency, so
//! it can be tested and reasoned about independently of the adapter that
//! chooses a word list.

use num_bigint::BigUint;
use thiserror::Error;

/// Errors raised while encoding a seed as a word name.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SeedNameError {
    /// The supplied dictionary was empty, which would make the radix zero.
    #[error("seed-name dictionary is empty")]
    EmptyDictionary,
    /// The supplied dictionary held a single word, which would make the radix
    /// one: dividing by one never shrinks the number, so the conversion would
    /// never end.
    #[error("seed-name dictionary must hold at least two words (got {len})")]
    DictionaryTooShort {
        /// Number of words supplied.
        len: usize,
    },
    /// A computed base-N digit fell outside the supplied dictionary.
    #[error("seed-name word index {index} is outside a {len}-word dictionary")]
    IndexOutOfRange {
        /// Index that was computed.
        index: usize,
        /// Number of words available.
        len: usize,
    },
}

/// Encodes a seed as a word name using the supplied dictionary.
///
/// The encoding is the one the retired `base-d` dependency produced, because
/// seed names are a compatibility surface. The seed is rendered as a decimal
/// string, those bytes are read as one big-endian integer, and that integer is
/// converted to base `dictionary.len()` by repeated division. Words are emitted
/// most significant first and joined with `-`.
///
/// Note that this is decimal-byte input, not a byte-oriented base such as
/// base64: the decimal string `"0"` is the byte `0x30`, not a string of zero
/// bytes, so a seed of `0` encodes the integer 48 and never yields an empty
/// name.
///
/// The word list is a parameter rather than a lookup so that the order is
/// pinned by the caller: reordering or swapping the provider renames every
/// existing seed.
///
/// # Errors
///
/// Returns [`SeedNameError::EmptyDictionary`] when the dictionary is empty,
/// which would otherwise divide by zero,
/// [`SeedNameError::DictionaryTooShort`] when it holds a single word, whose
/// radix of one would make the conversion loop forever, or
/// [`SeedNameError::IndexOutOfRange`] when a computed index falls outside it.
///
/// # Example
///
/// ```
/// use example_data::seed_name_from_value;
///
/// let words = ["alpha", "bravo", "charlie"];
/// let name = seed_name_from_value(2026, &words).expect("name should encode");
/// assert!(!name.is_empty());
/// ```
pub fn seed_name_from_value(seed: u64, dictionary: &[&str]) -> Result<String, SeedNameError> {
    match dictionary.len() {
        0 => return Err(SeedNameError::EmptyDictionary),
        1 => return Err(SeedNameError::DictionaryTooShort { len: 1 }),
        _ => {}
    }
    let radix = BigUint::from(dictionary.len());

    // Keep the original encoding of decimal UTF-8 bytes so existing seeds keep
    // their names when the word-list provider changes.
    let mut number = BigUint::from_bytes_be(seed.to_string().as_bytes());
    let mut words = Vec::new();
    while number != BigUint::from(0u8) {
        let remainder = (&number % &radix).to_u32_digits();
        let index = remainder.first().copied().unwrap_or(0) as usize;
        let word = dictionary
            .get(index)
            .ok_or(SeedNameError::IndexOutOfRange {
                index,
                len: dictionary.len(),
            })?;
        words.push(*word);
        number /= &radix;
    }
    words.reverse();
    Ok(words.join("-"))
}

#[cfg(test)]
mod tests;
