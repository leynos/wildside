//! Opaque cursor encoding and decoding helpers.

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use thiserror::Error;

/// Cursor wrapper for an ordered boundary key.
///
/// The encoded representation is base64url JSON and must be treated as opaque
/// by clients.
///
/// # Example
///
/// ```
/// use pagination::Cursor;
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
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cursor<Key> {
    key: Key,
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
    /// Construct a cursor from one ordering key.
    #[must_use]
    pub const fn new(key: Key) -> Self {
        Self { key }
    }

    /// Borrow the cursor key.
    #[must_use]
    pub const fn key(&self) -> &Key {
        &self.key
    }

    /// Consume the cursor and return the inner key.
    #[must_use]
    pub fn into_inner(self) -> Key {
        self.key
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
        let payload =
            URL_SAFE_NO_PAD
                .decode(value)
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
    use serde::{Deserialize, Serialize};

    use super::{Cursor, CursorError};

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct FixtureKey {
        created_at: String,
        id: String,
    }

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
    fn structurally_invalid_json_cursor_fails_decode() {
        let invalid_payload =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(br#"{"unexpected":true}"#);

        let result = Cursor::<FixtureKey>::decode(&invalid_payload);

        assert!(matches!(result, Err(CursorError::Deserialize { .. })));
    }
}
