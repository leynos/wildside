//! Opaque cursor encoding and decoding helpers.
//!
//! This module provides the [`Cursor<Key>`] type for encoding pagination
//! positions as opaque base64url-encoded JSON tokens, and the [`Direction`]
//! enum for bidirectional navigation. Cursors are transport-neutral and can be
//! used with any HTTP framework or serialization format.
//!
//! The base64url JSON encoding format ensures cursors are URL-safe and do not
//! require additional escaping in query parameters. The `dir` field is optional
//! in the JSON representation for backward compatibility with clients that omit
//! it (the default direction is `Next`).
//!
//! **Security consideration**: cursors are opaque but not signed or encrypted.
//! They encode the ordering key only, not any access control information.
//! Consumers must validate that the requesting user has permission to access
//! the underlying data.

use base64::{
    Engine as _,
    engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD},
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use thiserror::Error;

/// Direction of pagination relative to the cursor.
///
/// Indicates whether the cursor represents a position for fetching
/// the next page (forward in sort order) or the previous page
/// (backward in sort order).
///
/// # Examples
///
/// ```
/// use pagination::Direction;
///
/// let forward = Direction::Next;
/// let backward = Direction::Prev;
///
/// assert_ne!(forward, backward);
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Direction {
    /// Forward in the sort order (e.g., newer items if sorting ascending).
    #[default]
    Next,
    /// Backward in the sort order (e.g., older items).
    Prev,
}

/// Cursor wrapper for an ordered boundary key with direction.
///
/// The encoded representation is base64url JSON and must be treated as opaque
/// by clients. The direction indicates whether this cursor is meant for
/// fetching the next page (forward) or previous page (backward).
///
/// # Example
///
/// ```
/// use pagination::{Cursor, Direction};
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// struct UserKey {
///     created_at: String,
///     id: String,
/// }
///
/// let cursor = Cursor::new(UserKey {
///     created_at: "2026-03-22T10:30:00Z".to_owned(),
///     id: "8b116c56-0a58-4c55-b7d7-06ee6bbddb8c".to_owned(),
/// });
/// let encoded = cursor.encode().expect("cursor encoding succeeds");
/// let decoded = Cursor::<UserKey>::decode(&encoded).expect("cursor decoding succeeds");
///
/// assert_eq!(decoded.key(), cursor.key());
/// assert_eq!(decoded.direction(), Direction::Next);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cursor<Key> {
    key: Key,
    #[serde(default)]
    dir: Direction,
}

/// Errors raised while encoding or decoding opaque cursors.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CursorError {
    /// The cursor key could not be serialized into JSON.
    #[error("cursor JSON serialization failed: {message}")]
    Serialize {
        /// Human-readable serialization failure details.
        message: String,
    },
    /// The encoded token was not valid base64url.
    #[error("cursor is not valid base64url: {message}")]
    InvalidBase64 {
        /// Human-readable base64 decoding failure details.
        message: String,
    },
    /// The decoded JSON payload did not match the expected key shape.
    #[error("cursor JSON deserialization failed: {message}")]
    Deserialize {
        /// Human-readable deserialization failure details.
        message: String,
    },
}

impl<Key> Cursor<Key> {
    /// Construct a cursor from one ordering key with the default direction (`Next`).
    ///
    /// # Examples
    ///
    /// ```
    /// use pagination::{Cursor, Direction};
    ///
    /// let cursor = Cursor::new("my-key");
    /// assert_eq!(cursor.direction(), Direction::Next);
    /// ```
    #[must_use]
    pub const fn new(key: Key) -> Self {
        Self {
            key,
            dir: Direction::Next,
        }
    }

    /// Construct a cursor from one ordering key with an explicit direction.
    ///
    /// # Examples
    ///
    /// ```
    /// use pagination::{Cursor, Direction};
    ///
    /// let cursor = Cursor::with_direction("my-key", Direction::Prev);
    /// assert_eq!(cursor.direction(), Direction::Prev);
    /// ```
    #[must_use]
    pub const fn with_direction(key: Key, dir: Direction) -> Self {
        Self { key, dir }
    }

    /// Borrow the cursor key.
    #[must_use]
    pub const fn key(&self) -> &Key {
        &self.key
    }

    /// Access the pagination direction.
    #[must_use]
    pub const fn direction(&self) -> Direction {
        self.dir
    }

    /// Consume the cursor and return the inner key.
    #[must_use]
    pub fn into_inner(self) -> Key {
        self.key
    }

    /// Decompose the cursor into its constituent parts (key and direction).
    ///
    /// # Examples
    ///
    /// ```
    /// use pagination::{Cursor, Direction};
    ///
    /// let cursor = Cursor::with_direction("my-key", Direction::Prev);
    /// let (key, dir) = cursor.into_parts();
    /// assert_eq!(key, "my-key");
    /// assert_eq!(dir, Direction::Prev);
    /// ```
    #[must_use]
    pub fn into_parts(self) -> (Key, Direction) {
        (self.key, self.dir)
    }
}

impl<Key> Cursor<Key>
where
    Key: Serialize,
{
    /// Encode the cursor to an opaque base64url JSON token.
    ///
    /// # Errors
    ///
    /// Returns [`CursorError::Serialize`] when the cursor key cannot be
    /// serialized into JSON.
    pub fn encode(&self) -> Result<String, CursorError> {
        let payload = serde_json::to_vec(self).map_err(|error| CursorError::Serialize {
            message: error.to_string(),
        })?;
        Ok(URL_SAFE_NO_PAD.encode(payload))
    }
}

impl<Key> Cursor<Key>
where
    Key: DeserializeOwned,
{
    /// Decode a cursor from an opaque base64url JSON token.
    ///
    /// # Errors
    ///
    /// Returns [`CursorError::InvalidBase64`] when `value` is not valid
    /// base64url and [`CursorError::Deserialize`] when the decoded JSON does
    /// not match the expected cursor shape.
    pub fn decode(value: &str) -> Result<Self, CursorError> {
        let payload = URL_SAFE_NO_PAD
            .decode(value)
            .or_else(|_| URL_SAFE.decode(value))
            .map_err(|error| CursorError::InvalidBase64 {
                message: error.to_string(),
            })?;
        serde_json::from_slice(&payload).map_err(|error| CursorError::Deserialize {
            message: error.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for opaque cursor encoding and decoding.

    use base64::Engine as _;
    use rstest::rstest;
    use serde::{Deserialize, Serialize};

    use super::{Cursor, CursorError, Direction};

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct FixtureKey {
        created_at: String,
        id: String,
    }

    // Verify Cursor constructors work in const contexts
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
        let decoded =
            Cursor::<FixtureKey>::decode(&encoded).expect("cursor decoding should succeed");

        assert_eq!(decoded, cursor);
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
        // Simulate an old cursor (pre-4.1.2) without the `dir` field
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
        let json_value: serde_json::Value =
            serde_json::from_slice(&decoded_bytes).expect("valid JSON");

        // Verify the direction field exists and has the expected value
        let dir_value = json_value
            .get("dir")
            .and_then(|v| v.as_str())
            .expect("dir field should exist and be a string");
        assert_eq!(dir_value, expected);
    }

    #[test]
    fn invalid_direction_value_returns_deserialize_error() {
        // Create a cursor JSON with an invalid "dir" value
        let invalid_cursor_json =
            r#"{"key":{"created_at":"2026-03-22T10:30:00Z","id":"test-id"},"dir":"Sideways"}"#;
        let encoded = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(invalid_cursor_json);

        let result = Cursor::<FixtureKey>::decode(&encoded);

        assert!(matches!(result, Err(CursorError::Deserialize { .. })));
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
}
