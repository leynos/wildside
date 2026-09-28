//! Guards generated payloads and V-8 edit forms against vacuous coverage.

use proptest::{
    prelude::*,
    strategy::ValueTree,
    test_runner::{Config, RngSeed, TestRunner},
};
use serde_json::Value;

use super::strategies::{SingleLeafEdit, payload_pair_with_single_leaf_edit, route_payload};
use crate::domain::ports::cache_key::{
    MAX_COORDINATE_MAGNITUDE, ROUNDED_COORDINATE_KEYS, SORTED_ARRAY_KEYS,
};

const SAMPLE_COUNT: usize = 512;
const COVERAGE_SEED: u64 = 0x51_04_2a;

#[test]
fn generated_strategies_reach_special_keys_and_all_v8_edits() {
    let mut runner = TestRunner::new(Config {
        cases: SAMPLE_COUNT as u32,
        rng_seed: RngSeed::Fixed(COVERAGE_SEED),
        ..Config::default()
    });
    let strategy = route_payload();
    let mut observed = ObservedPayloadFeatures::default();

    for _ in 0..SAMPLE_COUNT {
        let payload = strategy
            .new_tree(&mut runner)
            .expect("route payload strategy should generate a value")
            .current();
        observe_payload_features(&payload, None, &mut observed);
    }

    assert!(
        observed.has_theme_array,
        "fixed-seed samples missed an all-string theme array: {observed:?}"
    );
    assert!(
        observed.has_fractional_coordinate,
        "fixed-seed samples missed an in-range fractional coordinate: {observed:?}"
    );
    assert!(
        observed.has_out_of_range_coordinate,
        "fixed-seed samples missed an out-of-range coordinate: {observed:?}"
    );

    let edits = payload_pair_with_single_leaf_edit();
    for _ in 0..SAMPLE_COUNT {
        let (edit, _, _) = edits
            .new_tree(&mut runner)
            .expect("single-leaf strategy should generate a pair")
            .current();
        observed.edits.observe(edit);
    }

    assert!(
        observed.edits.has_string_value_change,
        "fixed-seed samples missed a string-value edit: {:?}",
        observed.edits
    );
    assert!(
        observed.edits.has_boolean_flip,
        "fixed-seed samples missed a boolean flip: {:?}",
        observed.edits
    );
    assert!(
        observed.edits.has_fresh_key_insertion,
        "fixed-seed samples missed a fresh-key insertion: {:?}",
        observed.edits
    );
}

#[derive(Debug, Default)]
struct ObservedPayloadFeatures {
    has_theme_array: bool,
    has_fractional_coordinate: bool,
    has_out_of_range_coordinate: bool,
    edits: ObservedSingleLeafEdits,
}

#[derive(Debug, Default)]
struct ObservedSingleLeafEdits {
    has_string_value_change: bool,
    has_boolean_flip: bool,
    has_fresh_key_insertion: bool,
}

impl ObservedSingleLeafEdits {
    fn observe(&mut self, edit: SingleLeafEdit) {
        match edit {
            SingleLeafEdit::StringValueChanged => self.has_string_value_change = true,
            SingleLeafEdit::BooleanFlipped => self.has_boolean_flip = true,
            SingleLeafEdit::FreshKeyInserted => self.has_fresh_key_insertion = true,
        }
    }
}

fn observe_payload_features(
    value: &Value,
    current_key: Option<&str>,
    observed: &mut ObservedPayloadFeatures,
) {
    match value {
        Value::Number(number)
            if current_key.is_some_and(|key| ROUNDED_COORDINATE_KEYS.contains(&key)) =>
        {
            if let Some(value) = number.as_f64() {
                observed.has_fractional_coordinate |=
                    value.abs() <= MAX_COORDINATE_MAGNITUDE && value.fract() != 0.0;
                observed.has_out_of_range_coordinate |= value.abs() > MAX_COORDINATE_MAGNITUDE;
            }
        }
        Value::Object(entries) => {
            for (key, child) in entries {
                observe_payload_features(child, Some(key), observed);
            }
        }
        Value::Array(items) => {
            observed.has_theme_array |= current_key
                .is_some_and(|key| SORTED_ARRAY_KEYS.contains(&key))
                && items.len() >= 2
                && items.iter().all(Value::is_string);

            for item in items {
                observe_payload_features(item, None, observed);
            }
        }
        _ => {}
    }
}
