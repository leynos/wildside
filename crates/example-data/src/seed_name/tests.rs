//! Unit tests for the seed-name encoder's boundaries and error paths.
//!
//! The compatibility of full-size names with the retired `base-d` encoder is
//! held by `tests/properties/seed_names.rs`; these tests pin the small
//! dictionaries and the inputs the property oracle does not generate.

use rstest::rstest;

use super::{SeedNameError, seed_name_from_value};

fn numbered_words(count: usize) -> Vec<String> {
    (0..count).map(|index| format!("w{index}")).collect()
}

#[rstest]
fn an_empty_dictionary_is_rejected() {
    assert_eq!(
        seed_name_from_value(7, &[]),
        Err(SeedNameError::EmptyDictionary)
    );
}

#[rstest]
#[case::any_seed(7)]
#[case::zero(0)]
#[case::largest(u64::MAX)]
fn a_one_word_dictionary_is_rejected_instead_of_looping(#[case] seed: u64) {
    // A radix of one never shrinks the number, so an unchecked encoder would
    // never return for any seed.
    assert_eq!(
        seed_name_from_value(seed, &["alpha"]),
        Err(SeedNameError::DictionaryTooShort { len: 1 })
    );
}

#[rstest]
#[case::zero(0, "b-b-a-a-a-a")]
#[case::single_digit_seed(7, "b-b-a-b-b-b")]
fn a_two_word_dictionary_encodes_the_decimal_bytes_in_binary(
    #[case] seed: u64,
    #[case] expected: &str,
) {
    // The smallest accepted dictionary terminates: "0" is the byte 0x30, which
    // is 110000 in binary, most significant digit first.
    let name = seed_name_from_value(seed, &["a", "b"]).expect("two words are enough");
    assert_eq!(name, expected);
}

#[rstest]
fn seed_zero_encodes_the_byte_0x30_rather_than_an_empty_name() {
    // The decimal string "0" is the integer 48, so a dictionary longer than 48
    // words names it with the single word at index 48.
    let words = numbered_words(100);
    let dictionary: Vec<&str> = words.iter().map(String::as_str).collect();
    assert_eq!(
        seed_name_from_value(0, &dictionary).expect("name should encode"),
        "w48"
    );
}

#[rstest]
fn a_three_word_dictionary_names_a_seed_most_significant_word_first() {
    // 7 is the byte 0x37, which is 55: digits 2, 0, 0, 1 in base three.
    let name = seed_name_from_value(7, &["alpha", "bravo", "charlie"]).expect("name encodes");
    assert_eq!(name, "charlie-alpha-alpha-bravo");
}
