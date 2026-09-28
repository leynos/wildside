//! Shared strategies for route-request and cache-key properties.

use proptest::prelude::*;
use serde_json::{Map, Number, Value};

const GRID_CELLS: i64 = 18_000_000;
const COORDINATE_SCALE: f64 = 100_000.0;
const LARGE_COORDINATE_MIN: f64 = 22_517_998_136.852_48;

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
    recursive_route_payload(admitted_coordinate_number())
}

pub(super) fn route_payload() -> impl Strategy<Value = Value> {
    let recursive = recursive_route_payload(coordinate_number());
    let coordinate = coordinate_number().prop_map(|value| serde_json::json!({"lat": value}));

    // Ensure the broad numeric domain is exercised at a coordinate leaf, even
    // when recursive JSON generation chooses unrelated object keys.
    prop_oneof![2 => recursive, 2 => coordinate]
}

fn recursive_route_payload<C>(coordinate_values: C) -> impl Strategy<Value = Value> + 'static
where
    C: Strategy<Value = Value> + Clone + 'static,
{
    json_leaf().prop_recursive(4, 128, 8, move |inner| {
        let coordinate_values = coordinate_values.clone();
        let theme_entry = (theme_array_key(), theme_array()).prop_map(|(key, themes)| {
            (
                key,
                Value::Array(themes.into_iter().map(Value::String).collect()),
            )
        });
        let entry = prop_oneof![
            4 => (object_key(), inner.clone()),
            3 => (coordinate_key(), coordinate_values),
            2 => theme_entry,
        ];
        let object = prop::collection::vec(entry, 0..=8).prop_map(object_value);
        let array = prop::collection::vec(inner, 0..=8).prop_map(Value::Array);

        prop_oneof![object, array]
    })
}

fn coordinate_number() -> impl Strategy<Value = Value> + Clone {
    prop_oneof![
        2 => any::<i64>().prop_map(|value| Value::Number(value.into())),
        2 => any::<u64>().prop_map(|value| Value::Number(value.into())),
        4 => finite_json_number(),
        2 => large_coordinate_number(),
    ]
}

pub(super) fn distinct_integer_pair() -> impl Strategy<Value = (Value, Value)> {
    prop_oneof![
        (any::<i64>(), 1_u64..=1024).prop_map(|(first, delta)| {
            let second = first.wrapping_add(delta as i64);
            (Value::Number(first.into()), Value::Number(second.into()))
        }),
        (any::<u64>(), 1_u64..=1024).prop_map(|(first, delta)| {
            let second = first.wrapping_add(delta);
            (Value::Number(first.into()), Value::Number(second.into()))
        }),
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SingleLeafEdit {
    StringValueChanged,
    BooleanFlipped,
    FreshKeyInserted,
}

pub(super) fn payload_pair_with_single_leaf_edit()
-> impl Strategy<Value = (SingleLeafEdit, Value, Value)> {
    let string_edit = (admitted_route_payload(), small_string(), small_string()).prop_map(
        |(sample, leaf_name, original_leaf)| {
            let leaf_key = format!("field-{leaf_name}");
            let mut original_content = content_with_sample(sample);
            original_content.insert(leaf_key.clone(), Value::String(original_leaf.clone()));

            let mut edited_content = original_content.clone();
            edited_content.insert(leaf_key, Value::String(format!("{original_leaf}!")));

            tagged_edit_pair(
                SingleLeafEdit::StringValueChanged,
                request_pair_with_content_edit(original_content, edited_content, original_leaf),
            )
        },
    );

    let boolean_flip = (admitted_route_payload(), small_string()).prop_map(|(sample, sibling)| {
        let mut original_content = content_with_sample(sample);
        original_content.insert("enabled".to_owned(), Value::Bool(true));

        let mut edited_content = original_content.clone();
        edited_content.insert("enabled".to_owned(), Value::Bool(false));

        tagged_edit_pair(
            SingleLeafEdit::BooleanFlipped,
            request_pair_with_content_edit(original_content, edited_content, sibling),
        )
    });

    let fresh_key_insertion = (admitted_route_payload(), small_string(), small_string()).prop_map(
        |(sample, key_suffix, sibling)| {
            let original_content = content_with_sample(sample);
            let mut edited_content = original_content.clone();
            edited_content.insert(
                format!("fresh-{key_suffix}"),
                Value::String(sibling.clone()),
            );

            tagged_edit_pair(
                SingleLeafEdit::FreshKeyInserted,
                request_pair_with_content_edit(original_content, edited_content, sibling),
            )
        },
    );

    prop_oneof![1 => string_edit, 1 => boolean_flip, 1 => fresh_key_insertion]
}

fn content_with_sample(sample: Value) -> Map<String, Value> {
    Map::from_iter([("sample".to_owned(), sample)])
}

fn tagged_edit_pair(
    edit: SingleLeafEdit,
    (original, edited): (Value, Value),
) -> (SingleLeafEdit, Value, Value) {
    (edit, original, edited)
}

fn request_pair_with_content_edit(
    original_content: Map<String, Value>,
    edited_content: Map<String, Value>,
    sibling: String,
) -> (Value, Value) {
    let request = |content| {
        serde_json::json!({
            "generated": {
                "payload": {
                    "content": Value::Object(content),
                    "edited_leaf": sibling.clone(),
                },
                "property_edit": sibling.clone(),
            },
        })
    };

    (request(original_content), request(edited_content))
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

fn finite_json_number() -> impl Strategy<Value = Value> + Clone {
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

fn large_coordinate_number() -> impl Strategy<Value = Value> + Clone {
    prop_oneof![
        (LARGE_COORDINATE_MIN..=1.0e12_f64)
            .prop_filter_map("large positive coordinates are JSON numbers", json_number),
        (-1.0e12_f64..=-LARGE_COORDINATE_MIN)
            .prop_filter_map("large negative coordinates are JSON numbers", json_number),
    ]
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

fn theme_array_key() -> impl Strategy<Value = String> {
    prop::sample::select(vec![
        "themes".to_owned(),
        "themeIds".to_owned(),
        "interestThemeIds".to_owned(),
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
