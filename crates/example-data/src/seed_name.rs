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
    /// The supplied dictionary had fewer than two words.
    ///
    /// Radix one is not a base: `number /= radix` would leave the value
    /// unchanged, so the digit loop would never terminate.
    #[error("seed-name dictionary needs at least two words, got {len}")]
    DictionaryTooSmall {
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
/// base64: a seed of `0` yields the single word at index zero, because the
/// decimal string `"0"` is not a string of zero bytes.
///
/// The word list is a parameter rather than a lookup so that the order is
/// pinned by the caller: reordering or swapping the provider renames every
/// existing seed.
///
/// # Errors
///
/// Returns [`SeedNameError::DictionaryTooSmall`] when the dictionary has fewer
/// than two words, which would otherwise divide by zero or never terminate the
/// digit loop, or
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
    if dictionary.len() < 2 {
        return Err(SeedNameError::DictionaryTooSmall {
            len: dictionary.len(),
        });
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
mod tests {
    //! Coverage for the encoder's error paths and digit ordering.

    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::empty(&[], 0)]
    #[case::single_word(&["solo"], 1)]
    fn a_dictionary_that_is_not_a_base_is_rejected(
        #[case] dictionary: &[&str],
        #[case] expected_len: usize,
    ) {
        // Radix one would divide by one forever, so this must fail rather than
        // hang. The length is carried so the message can name the real input.
        assert_eq!(
            seed_name_from_value(2026, dictionary),
            Err(SeedNameError::DictionaryTooSmall { len: expected_len })
        );
    }

    #[test]
    fn seed_zero_still_encodes_a_multi_digit_name() {
        // The decimal string "0" is the single byte 0x30, read as the integer
        // 48, which in base 3 is 1,2,1,0. A byte-oriented base such as base64
        // would instead treat seed 0 as no bytes at all and emit nothing, so
        // this pins the decimal-bytes reading that kept the retired `base-d`
        // names.
        let dictionary = ["first", "second", "third"];
        assert_eq!(
            seed_name_from_value(0, &dictionary).expect("seed 0 encodes"),
            "second-third-second-first"
        );
    }

    #[test]
    fn digits_are_emitted_most_significant_first() {
        // "2026" is the bytes 0x32 0x30 0x32 0x36, which as a big-endian
        // integer is 842_019_382. Its base-3 digits are
        // 2,0,1,1,2,0,0,1,0,2,0,0,0,1,1,0,0,0,1, emitted with the highest
        // power of three first.
        let dictionary = ["a", "b", "c"];
        let expected = "c-a-b-b-c-a-a-b-a-c-a-a-a-b-b-a-a-a-b";
        assert_eq!(
            seed_name_from_value(2026, &dictionary).expect("seed 2026 encodes"),
            expected
        );
    }

    #[test]
    fn every_digit_stays_inside_the_dictionary() {
        // A dictionary of length n can only ever yield digits below n, so a
        // short list is not a source of out-of-range indices for any seed.
        let dictionary = ["x", "y"];
        let name = seed_name_from_value(u64::MAX, &dictionary).expect("wider seed encodes");
        assert!(
            name.split('-').all(|word| dictionary.contains(&word)),
            "unexpected digit in {name}"
        );
    }
}
