//! Shared strategies for route-request and cache-key properties.

use proptest::prelude::*;
use serde_json::{Map, Number, Value};

const GRID_CELLS: i64 = 18_000_000;
const COORDINATE_SCALE: f64 = 100_000.0;

pub(super) fn theme_array() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec(0_u8..4, 0..=8).prop_map(|values| {
        values
            .into_iter()
            .map(|value| format!("theme-{value}"))
            .collect()
    })
}

pub(super) fn grid_cell_pair() -> impl Strategy<Value = (f64, f64)> {
    let cells = prop_oneof![
        4 => -GRID_CELLS..=-1_i64,
        4 => 1_i64..=GRID_CELLS,
        1 => Just(0_i64),
    ];

    (cells, 0.01_f64..0.49_f64).prop_map(|(cell, offset)| {
        (
            coordinate_for_offset(cell, -offset),
            coordinate_for_offset(cell, offset),
        )
    })
}

pub(super) fn divergent_cells() -> impl Strategy<Value = (i64, i64)> {
    let delta = prop_oneof![-2 * GRID_CELLS..=-1_i64, 1_i64..=2 * GRID_CELLS,];

    (-GRID_CELLS..=GRID_CELLS, delta).prop_map(|(cell, delta)| {
        let candidate = (cell + delta).clamp(-GRID_CELLS, GRID_CELLS);
        let distinct_candidate = if candidate == cell {
            if cell == GRID_CELLS {
                cell - 1
            } else {
                cell + 1
            }
        } else {
            candidate
        };
        (cell, distinct_candidate)
    })
}

pub(super) fn admitted_route_payload() -> impl Strategy<Value = Value> {
    let coordinate_values = admitted_coordinate_number();

    json_leaf().prop_recursive(4, 128, 8, move |inner| {
        let coordinate_values = coordinate_values.clone();
        let entry = prop_oneof![
            4 => (object_key(), inner.clone()),
            3 => (coordinate_key(), coordinate_values),
        ];
        let object = prop::collection::vec(entry, 0..=8).prop_map(object_value);
        let array = prop::collection::vec(inner, 0..=8).prop_map(Value::Array);

        prop_oneof![object, array]
    })
}

pub(super) fn payload_pair_with_single_leaf_edit() -> impl Strategy<Value = (Value, Value)> {
    (admitted_route_payload(), small_string(), small_string()).prop_map(
        |(sample, leaf_name, original_leaf)| {
            let leaf_key = format!("field-{leaf_name}");
            let mut original_content = Map::new();
            original_content.insert("sample".to_owned(), sample);
            original_content.insert(leaf_key.clone(), Value::String(original_leaf.clone()));

            let mut edited_content = original_content.clone();
            edited_content.insert(leaf_key, Value::String(format!("{original_leaf}!")));

            let request = |content| {
                serde_json::json!({
                    "generated": {
                        "payload": {
                            "content": Value::Object(content),
                            "edited_leaf": original_leaf,
                        },
                        "property_edit": original_leaf,
                    },
                })
            };

            (request(original_content), request(edited_content))
        },
    )
}

pub(super) fn small_string() -> impl Strategy<Value = String> {
    prop::collection::vec(0_u8..26, 0..=12).prop_map(|letters| {
        letters
            .into_iter()
            .map(|letter| char::from(b'a' + letter))
            .collect()
    })
}

pub(super) fn rotate_by_one<T: Clone>(values: &[T]) -> Vec<T> {
    let mut rotated = values.to_vec();
    rotated.rotate_left(1);
    rotated
}

fn finite_json_number() -> impl Strategy<Value = Value> {
    prop_oneof![
        prop::num::f64::NORMAL | prop::num::f64::POSITIVE,
        prop::num::f64::NORMAL | prop::num::f64::NEGATIVE,
        prop::num::f64::SUBNORMAL | prop::num::f64::POSITIVE,
        prop::num::f64::SUBNORMAL | prop::num::f64::NEGATIVE,
        prop::num::f64::ZERO | prop::num::f64::POSITIVE,
        prop::num::f64::ZERO | prop::num::f64::NEGATIVE,
    ]
    .prop_filter_map("finite floats are JSON numbers", json_number)
}

fn admitted_coordinate_number() -> impl Strategy<Value = Value> + Clone {
    prop_oneof![
        2 => (-180.0_f64..=180.0_f64)
            .prop_filter_map("in-range coordinates are JSON numbers", json_number),
        1 => (-180_i64..=180_i64).prop_map(|value| Value::Number(value.into())),
        1 => (0_u64..=180_u64).prop_map(|value| Value::Number(value.into())),
    ]
}

fn json_leaf() -> impl Strategy<Value = Value> {
    prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(|value| Value::Number(value.into())),
        any::<u64>().prop_map(|value| Value::Number(value.into())),
        finite_json_number(),
        small_string().prop_map(Value::String),
    ]
}

fn object_key() -> impl Strategy<Value = String> {
    prop_oneof![
        5 => prop::sample::select(vec![
            "themes".to_owned(),
            "themeIds".to_owned(),
            "interestThemeIds".to_owned(),
        ]),
        3 => small_string().prop_map(|key| format!("field-{key}")),
    ]
}

fn coordinate_key() -> impl Strategy<Value = String> {
    prop::sample::select(vec![
        "lat".to_owned(),
        "lng".to_owned(),
        "lon".to_owned(),
        "latitude".to_owned(),
        "longitude".to_owned(),
    ])
}

fn json_number(value: f64) -> Option<Value> {
    Number::from_f64(value).map(Value::Number)
}

fn object_value(entries: Vec<(String, Value)>) -> Value {
    Value::Object(entries.into_iter().collect::<Map<_, _>>())
}

fn coordinate_for_offset(cell: i64, offset: f64) -> f64 {
    ((cell as f64 + offset) / COORDINATE_SCALE).clamp(-180.0, 180.0)
}
