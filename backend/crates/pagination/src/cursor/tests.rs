//! Unit tests for opaque cursor encoding and decoding.

use base64::Engine as _;
use insta::assert_json_snapshot;
use proptest::{prelude::Just, prop_oneof, proptest, string::string_regex};
use rstest::rstest;
use serde::{Deserialize, Serialize};

use super::{Cursor, CursorError, Direction};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct FixtureKey {
    created_at: String,
    id: String,
}

// Verify Cursor constructors work in const contexts.
const _CONST_CURSOR: Cursor<&str> = Cursor::new("compile-time-test");
const _CONST_DIRECTIONAL_CURSOR: Cursor<&str> =
    Cursor::with_direction("compile-time-test", Direction::Prev);

#[test]
fn cursor_round_trips_through_opaque_token() {
    let cursor = Cursor::new(FixtureKey {
        created_at: "2026-03-22T10:30:00Z".to_owned(),
        id: "8b116c56-0a58-4c55-b7d7-06ee6bbddb8c".to_owned(),
    });

    let encoded = cursor.encode().expect("cursor encoding should succeed");
    let decoded = Cursor::<FixtureKey>::decode(&encoded).expect("cursor decoding should succeed");

    assert_eq!(decoded, cursor);
}

proptest! {
    #[test]
    fn round_trips_through_cursor_encode_decode(
        created_at in string_regex("[[:alnum:]-:.+T t]{1,32}")
            .expect("created_at strategy should parse"),
        id in string_regex("[[:alnum:]-_]{1,32}").expect("id strategy should parse"),
        direction in prop_oneof![Just(Direction::Next), Just(Direction::Prev)],
    ) {
        let cursor = Cursor::with_direction(
            FixtureKey {
                created_at,
                id,
            },
            direction,
        );

        let encoded = cursor.encode().expect("cursor encoding should succeed");
        let decoded = Cursor::<FixtureKey>::decode(&encoded)
            .expect("cursor decoding should succeed");

        assert_eq!(decoded, cursor);
    }
}

#[test]
fn invalid_base64_cursor_fails_decode() {
    let result = Cursor::<FixtureKey>::decode("!!!");

    assert!(matches!(result, Err(CursorError::InvalidBase64 { .. })));
}

#[test]
fn padded_base64_cursor_decodes_successfully() {
    let cursor = Cursor::new(FixtureKey {
        created_at: "2026-03-22T10:30:00Z".to_owned(),
        id: "8b116c56-0a58-4c55-b7d7-06ee6bbddb8c".to_owned(),
    });
    let payload = serde_json::to_vec(&cursor).expect("cursor should serialize");
    let encoded = base64::engine::general_purpose::URL_SAFE.encode(payload);

    let decoded =
        Cursor::<FixtureKey>::decode(&encoded).expect("padded cursor decoding should succeed");

    assert_eq!(decoded, cursor);
}

#[test]
fn structurally_invalid_json_cursor_fails_decode() {
    let invalid_payload =
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(br#"{"unexpected":true}"#);

    let result = Cursor::<FixtureKey>::decode(&invalid_payload);

    assert!(matches!(result, Err(CursorError::Deserialize { .. })));
}

#[rstest]
#[case(Direction::Next)]
#[case(Direction::Prev)]
fn direction_round_trips_through_encoding(#[case] direction: Direction) {
    let cursor = Cursor::with_direction(
        FixtureKey {
            created_at: "2026-03-22T10:30:00Z".to_owned(),
            id: "test-id".to_owned(),
        },
        direction,
    );
    let encoded = cursor.encode().expect("encoding succeeds");
    let decoded = Cursor::<FixtureKey>::decode(&encoded).expect("decoding succeeds");

    assert_eq!(decoded.direction(), direction);
    assert_eq!(decoded.key(), cursor.key());
}

#[test]
fn cursor_without_direction_defaults_to_next() {
    let old_cursor_json = r#"{"key":{"created_at":"2026-03-22T10:30:00Z","id":"test-id"}}"#;
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(old_cursor_json);

    let decoded = Cursor::<FixtureKey>::decode(&encoded).expect("decoding succeeds");

    assert_eq!(decoded.direction(), Direction::Next);
}

#[rstest]
#[case(Direction::Next, "Next")]
#[case(Direction::Prev, "Prev")]
fn new_cursor_includes_direction_in_json(#[case] direction: Direction, #[case] expected: &str) {
    let cursor = Cursor::with_direction(
        FixtureKey {
            created_at: "2026-03-22T10:30:00Z".to_owned(),
            id: "test-id".to_owned(),
        },
        direction,
    );
    let encoded = cursor.encode().expect("encoding succeeds");
    let decoded_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(&encoded)
        .expect("base64 decoding succeeds");
    let json_value: serde_json::Value = serde_json::from_slice(&decoded_bytes).expect("valid JSON");

    let dir_value = json_value
        .get("dir")
        .and_then(|v| v.as_str())
        .expect("dir field should exist and be a string");
    assert_eq!(dir_value, expected);
}

#[rstest]
#[case(Direction::Next, "cursor_wire_payload_next")]
#[case(Direction::Prev, "cursor_wire_payload_prev")]
fn cursor_json_payload_matches_snapshot(#[case] direction: Direction, #[case] snapshot_name: &str) {
    let cursor = Cursor::with_direction(
        FixtureKey {
            created_at: "2026-03-22T10:30:00Z".to_owned(),
            id: "test-id".to_owned(),
        },
        direction,
    );
    let encoded = cursor.encode().expect("encoding succeeds");
    let decoded_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(&encoded)
        .expect("base64 decoding succeeds");
    let json_value: serde_json::Value = serde_json::from_slice(&decoded_bytes).expect("valid JSON");

    assert_json_snapshot!(snapshot_name, json_value);
}

#[rstest]
#[case(
    r#"{"key":{"created_at":"2026-03-22T10:30:00Z","id":"test-id"},"dir":"Sideways"}"#,
    "Sideways"
)]
#[case(
    r#"{"key":{"created_at":"2026-03-22T10:30:00Z","id":"test-id"},"dir":123}"#,
    "123"
)]
#[case(
    r#"{"key":{"created_at":"2026-03-22T10:30:00Z","id":"test-id"},"dir":null}"#,
    "null"
)]
fn unsupported_direction_value_returns_unsupported_direction_error(
    #[case] cursor_json: &str,
    #[case] expected_direction: &str,
) {
    let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(cursor_json);

    let result = Cursor::<FixtureKey>::decode(&encoded);

    assert!(matches!(
        result,
        Err(CursorError::UnsupportedDirection { direction }) if direction == expected_direction
    ));
}

#[rstest]
#[case(Direction::Next)]
#[case(Direction::Prev)]
fn into_parts_returns_key_and_direction(#[case] direction: Direction) {
    let key = FixtureKey {
        created_at: "2026-03-22T10:30:00Z".to_owned(),
        id: "test-id".to_owned(),
    };
    let cursor = Cursor::with_direction(key.clone(), direction);

    let (returned_key, returned_dir) = cursor.into_parts();

    assert_eq!(returned_key, key);
    assert_eq!(returned_dir, direction);
}

#[test]
fn cursor_new_uses_next_direction() {
    let cursor = Cursor::new(FixtureKey {
        created_at: "2026-03-22T10:30:00Z".to_owned(),
        id: "test-id".to_owned(),
    });

    assert_eq!(cursor.direction(), Direction::Next);
}

#[test]
fn encode_returns_serialize_error_when_key_cannot_be_serialized() {
    use std::collections::HashMap;
    #[derive(Hash, PartialEq, Eq)]
    struct FailingKey;
    impl Serialize for FailingKey {
        fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
            Err(serde::ser::Error::custom("fail"))
        }
    }
    let cursor = Cursor {
        key: HashMap::from([(FailingKey, String::new())]),
        dir: Direction::Next,
    };
    let Err(CursorError::Serialize { message }) = cursor.encode() else {
        panic!("expected Serialize error")
    };
    assert!(message.contains("fail"));
}
