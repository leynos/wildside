//! Seed names must keep the encoding produced by the retired `base-d` encoder.
//!
//! The compatibility contract is range-wide: every `u64` seed must map to the
//! name that `base-d`'s `word::encode` produced from the seed's decimal UTF-8
//! bytes. The oracle below re-derives those names with schoolbook base-256
//! long division so it shares no code, and no big-integer library, with the
//! production encoder.
//!
//! Names cannot be split back into words on `-`: the EFF long list itself
//! contains hyphenated entries (`drop-down`, `felt-tip`, `t-shirt`, `yo-yo`).
//! Treat a name as a single opaque label and compare it whole.

use diceware_wordlists::Wordlist;
use example_data::seed_registry_cli::seed_name_for_seed;
use proptest::prelude::*;

/// Words in the EFF long list, in the provider's published order.
fn eff_long_words() -> &'static [&'static str] {
    Wordlist::EffLong.get_list()
}

/// Reference implementation of `base-d`'s decimal-byte big-endian radix
/// encoding.
///
/// The seed's decimal string is treated as base-256 limbs, most significant
/// first. Repeated long division by the dictionary size yields the word
/// indices from least significant to most significant, matching the original
/// encoder's reversed output order.
fn reference_name(seed: u64, words: &[&str]) -> Option<String> {
    let radix = u64::try_from(words.len()).ok()?;
    if radix == 0 {
        return None;
    }

    let mut limbs: Vec<u64> = seed.to_string().bytes().map(u64::from).collect();
    let mut reversed = Vec::new();

    while limbs.iter().any(|limb| *limb != 0) {
        let mut remainder = 0_u64;
        for limb in &mut limbs {
            let accumulator = (remainder << 8) | *limb;
            *limb = accumulator.checked_div(radix)?;
            remainder = accumulator.checked_rem(radix)?;
        }
        reversed.push(*words.get(usize::try_from(remainder).ok()?)?);
    }

    reversed.reverse();
    Some(reversed.join("-"))
}

proptest! {
    #[test]
    fn every_seed_keeps_the_base_d_name(seed in any::<u64>()) {
        let Some(expected) = reference_name(seed, eff_long_words()) else {
            return Err(TestCaseError::fail(format!(
                "reference encoder could not encode seed {seed}"
            )));
        };
        let actual = seed_name_for_seed(seed)
            .map_err(|err| TestCaseError::fail(err.to_string()))?;

        prop_assert_eq!(actual, expected);
    }

    #[test]
    fn generated_names_are_stable_lowercase_tokens(seed in any::<u64>()) {
        let name = seed_name_for_seed(seed)
            .map_err(|err| TestCaseError::fail(err.to_string()))?;
        let repeated = seed_name_for_seed(seed)
            .map_err(|err| TestCaseError::fail(err.to_string()))?;

        prop_assert!(!name.is_empty(), "seed {seed} produced an empty name");
        prop_assert_eq!(&name, &repeated, "seed {} is not deterministic", seed);
        prop_assert!(
            name.bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte == b'-'),
            "seed {seed} produced {name:?}, which is not lowercase or hyphenated",
        );
        prop_assert!(
            !name.starts_with('-') && !name.ends_with('-'),
            "seed {seed} produced a name with a dangling delimiter: {name:?}",
        );
    }
}
