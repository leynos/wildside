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
use serde::{Deserialize, Deserializer, Serialize, de::DeserializeOwned};
use thiserror::Error;
use tracing::debug;

/// Direction of pagination relative to the cursor.
///
/// Indicates whether the cursor represents a position for fetching
/// the next page (forward in sort order) or the previous page
/// (backward in sort order).
///
/// # Examples
///
/// ```rust
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
/// ```rust
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

#[derive(Debug, Deserialize)]
struct CursorWire<Key> {
    key: Key,
    #[serde(default, deserialize_with = "deserialize_wire_direction")]
    dir: WireDirection,
}

/// Tri-state view of the `dir` field needed to tell an absent field, an
/// explicit `null`, and a present value apart. `Option<Option<_>>` would
/// model the same domain but trips `clippy::option_option`.
#[derive(Debug, Default)]
enum WireDirection {
    #[default]
    Absent,
    Null,
    Present(serde_json::Value),
}

fn deserialize_wire_direction<'de, D>(deserializer: D) -> Result<WireDirection, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(if value.is_null() {
        WireDirection::Null
    } else {
        WireDirection::Present(value)
    })
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
    /// The decoded JSON payload used a direction this crate cannot navigate.
    #[error("cursor direction is unsupported: {direction}")]
    UnsupportedDirection {
        /// Direction value found in the cursor payload.
        direction: String,
    },
}

impl<Key> Cursor<Key> {
    /// Construct a cursor from one ordering key with the default direction (`Next`).
    ///
    /// # Examples
    ///
    /// ```rust
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
    /// ```rust
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
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pagination::{Cursor, Direction};
    ///
    /// let cursor = Cursor::with_direction("my-key", Direction::Prev);
    ///
    /// assert_eq!(cursor.key(), &"my-key");
    /// ```
    #[must_use]
    pub const fn key(&self) -> &Key {
        &self.key
    }

    /// Access the pagination direction.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pagination::{Cursor, Direction};
    ///
    /// let cursor = Cursor::with_direction("my-key", Direction::Prev);
    ///
    /// assert_eq!(cursor.direction(), Direction::Prev);
    /// ```
    #[must_use]
    pub const fn direction(&self) -> Direction {
        self.dir
    }

    /// Consume the cursor and return the inner key.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pagination::{Cursor, Direction};
    ///
    /// let cursor = Cursor::with_direction("my-key", Direction::Prev);
    ///
    /// assert_eq!(cursor.into_inner(), "my-key");
    /// ```
    #[must_use]
    pub fn into_inner(self) -> Key {
        self.key
    }

    /// Decompose the cursor into its constituent parts (key and direction).
    ///
    /// # Examples
    ///
    /// ```rust
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
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pagination::{Cursor, Direction};
    ///
    /// let cursor = Cursor::with_direction("my-key".to_owned(), Direction::Prev);
    ///
    /// let encoded = cursor.encode().expect("cursor encoding succeeds");
    ///
    /// assert!(!encoded.is_empty());
    /// assert!(!encoded.contains('='));
    /// ```
    ///
    /// ```rust
    /// use pagination::{Cursor, CursorError};
    /// use serde::Serialize;
    ///
    /// struct FailingKey;
    ///
    /// impl Serialize for FailingKey {
    ///     fn serialize<S>(&self, _: S) -> Result<S::Ok, S::Error>
    ///     where
    ///         S: serde::Serializer,
    ///     {
    ///         Err(serde::ser::Error::custom("cannot serialize key"))
    ///     }
    /// }
    ///
    /// let error = Cursor::new(FailingKey).encode().expect_err("encoding fails");
    ///
    /// assert!(matches!(error, CursorError::Serialize { .. }));
    /// ```
    pub fn encode(&self) -> Result<String, CursorError> {
        let payload = serde_json::to_vec(self).map_err(|error| {
            debug!(
                error_type = %std::any::type_name_of_val(&error),
                "cursor encoding failed"
            );
            CursorError::Serialize {
                message: error.to_string(),
            }
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
    /// base64url, [`CursorError::UnsupportedDirection`] when `dir` is present
    /// but not one of the supported directions, and [`CursorError::Deserialize`]
    /// when the decoded JSON does not match the expected cursor shape.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pagination::{Cursor, Direction};
    ///
    /// let cursor = Cursor::with_direction("my-key".to_owned(), Direction::Prev);
    /// let encoded = cursor.encode().expect("cursor encoding succeeds");
    ///
    /// let decoded = Cursor::<String>::decode(&encoded).expect("cursor decoding succeeds");
    ///
    /// assert_eq!(decoded, cursor);
    /// ```
    ///
    /// ```rust
    /// use pagination::{Cursor, CursorError};
    ///
    /// let error = Cursor::<String>::decode("not a cursor").expect_err("decoding fails");
    ///
    /// assert!(matches!(error, CursorError::InvalidBase64 { .. }));
    /// ```
    pub fn decode(value: &str) -> Result<Self, CursorError> {
        let payload = decode_base64_url(value)?;
        let wire: CursorWire<Key> = serde_json::from_slice(&payload).map_err(|error| {
            debug!(
                error_type = %std::any::type_name_of_val(&error),
                payload_len = payload.len(),
                "cursor JSON decoding failed"
            );
            CursorError::Deserialize {
                message: error.to_string(),
            }
        })?;
        Ok(Self {
            key: wire.key,
            dir: decode_direction(wire.dir)?,
        })
    }
}

fn decode_base64_url(value: &str) -> Result<Vec<u8>, CursorError> {
    URL_SAFE_NO_PAD
        .decode(value)
        .or_else(|_| URL_SAFE.decode(value))
        .map_err(|error| {
            debug!(
                error_type = %std::any::type_name_of_val(&error),
                token_len = value.len(),
                "cursor base64 decoding failed"
            );
            CursorError::InvalidBase64 {
                message: error.to_string(),
            }
        })
}

fn decode_direction(raw_direction: WireDirection) -> Result<Direction, CursorError> {
    match raw_direction {
        WireDirection::Absent => Ok(Direction::Next),
        WireDirection::Null => unsupported_direction("null".to_owned()),
        WireDirection::Present(value) => match value.as_str() {
            Some("Next") => Ok(Direction::Next),
            Some("Prev") => Ok(Direction::Prev),
            Some(direction) => unsupported_direction(direction.to_owned()),
            None => unsupported_direction(value.to_string()),
        },
    }
}

fn unsupported_direction(direction: String) -> Result<Direction, CursorError> {
    debug!(
        direction = %direction,
        "cursor direction validation failed"
    );
    Err(CursorError::UnsupportedDirection { direction })
}

#[cfg(test)]
mod tests;
