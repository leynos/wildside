//! Deterministic seed names using the EFF long word list.

use diceware_wordlists::Wordlist;
use num_bigint::BigUint;

use super::CliError;

pub(super) fn seed_name_from_value(seed: u64, dictionary: &[&str]) -> Result<String, CliError> {
    // Keep the original encoding of decimal UTF-8 bytes so existing seeds keep
    // their names when the word-list provider changes.
    let mut number = BigUint::from_bytes_be(seed.to_string().as_bytes());
    let radix = BigUint::from(dictionary.len());
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

pub(super) fn eff_long_dictionary() -> Result<&'static [&'static str], CliError> {
    let words = Wordlist::EffLong.get_list();
    if words.is_empty() {
        return Err(CliError::WordListError {
            message: "EFF word list is empty".to_owned(),
        });
    }
    Ok(words)
}
