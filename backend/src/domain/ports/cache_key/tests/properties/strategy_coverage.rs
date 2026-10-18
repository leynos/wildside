//! Guards generated payloads and V-8 edit forms against vacuous coverage.
//!
//! If a fixed-seed check fails after a proptest or strategy change, verify
//! witness reach and reseed; do not weaken the properties.

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
const ZERO_ROUNDING_CELL_BOUNDARY: f64 = 0.000_005;

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
        observed.has_admitted_fractional_coordinate_outside_zero_cell,
        "fixed-seed samples missed an admitted fractional coordinate outside the zero rounding cell: {observed:?}"
    );
    assert!(
        observed.has_out_of_range_coordinate,
        "fixed-seed samples missed an out-of-range coordinate: {observed:?}"
    );
    assert_non_numeric_coordinate_coverage(&observed);

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

fn assert_non_numeric_coordinate_coverage(observed: &ObservedPayloadFeatures) {
    assert!(
        observed.has_coordinate_string,
        "fixed-seed samples missed a string under a coordinate key: {observed:?}"
    );
    assert!(
        observed.has_coordinate_boolean,
        "fixed-seed samples missed a boolean under a coordinate key: {observed:?}"
    );
    assert!(
        observed.has_coordinate_null,
        "fixed-seed samples missed null under a coordinate key: {observed:?}"
    );
    assert!(
        observed.has_coordinate_array,
        "fixed-seed samples missed an array under a coordinate key: {observed:?}"
    );
    assert!(
        observed.has_coordinate_object,
        "fixed-seed samples missed an object under a coordinate key: {observed:?}"
    );
}

#[derive(Debug, Default)]
struct ObservedPayloadFeatures {
    has_theme_array: bool,
    has_admitted_fractional_coordinate_outside_zero_cell: bool,
    has_out_of_range_coordinate: bool,
    has_coordinate_string: bool,
    has_coordinate_boolean: bool,
    has_coordinate_null: bool,
    has_coordinate_array: bool,
    has_coordinate_object: bool,
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
    observe_non_numeric_coordinate_value(value, current_key, observed);

    match value {
        Value::Number(number) => observe_coordinate_feature(number, current_key, observed),
        Value::Object(entries) => {
            for (key, child) in entries {
                observe_payload_features(child, Some(key), observed);
            }
        }
        Value::Array(items) => observe_array_features(items, current_key, observed),
        _ => {}
    }
}

fn observe_non_numeric_coordinate_value(
    value: &Value,
    current_key: Option<&str>,
    observed: &mut ObservedPayloadFeatures,
) {
    if !current_key.is_some_and(|key| ROUNDED_COORDINATE_KEYS.contains(&key)) {
        return;
    }

    match value {
        Value::String(_) => observed.has_coordinate_string = true,
        Value::Bool(_) => observed.has_coordinate_boolean = true,
        Value::Null => observed.has_coordinate_null = true,
        Value::Array(_) => observed.has_coordinate_array = true,
        Value::Object(_) => observed.has_coordinate_object = true,
        Value::Number(_) => {}
    }
}

fn observe_coordinate_feature(
    number: &serde_json::Number,
    current_key: Option<&str>,
    observed: &mut ObservedPayloadFeatures,
) {
    if !current_key.is_some_and(|key| ROUNDED_COORDINATE_KEYS.contains(&key)) {
        return;
    }

    if let Some(value) = number.as_f64() {
        observed.has_admitted_fractional_coordinate_outside_zero_cell |= value.abs()
            <= MAX_COORDINATE_MAGNITUDE
            && value.abs() > ZERO_ROUNDING_CELL_BOUNDARY
            && value.fract() != 0.0;
        observed.has_out_of_range_coordinate |= value.abs() > MAX_COORDINATE_MAGNITUDE;
    }
}

fn observe_array_features(
    items: &[Value],
    current_key: Option<&str>,
    observed: &mut ObservedPayloadFeatures,
) {
    observed.has_theme_array |= is_theme_array(current_key, items);

    for item in items {
        observe_payload_features(item, None, observed);
    }
}

fn is_theme_array(current_key: Option<&str>, items: &[Value]) -> bool {
    current_key.is_some_and(|key| SORTED_ARRAY_KEYS.contains(&key))
        && items.len() >= 2
        && items.iter().all(Value::is_string)
}
